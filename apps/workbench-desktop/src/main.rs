#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::Manager;
mod avatar;
mod avatar_preview;
mod human;
mod obs;
use human::{
    MicService, workbench_all_mute, workbench_human_resume, workbench_human_start,
    workbench_mic_devices, workbench_mic_poll, workbench_mic_stop,
};
use obs::workbench_obs_observe;
#[derive(Clone, Default)]
struct PublicOutputOwner(
    Arc<Mutex<Option<nai_workbench_host::public_output::OutputOwner>>>,
    OutputSnapshot,
);
type OutputSnapshot = Arc<Mutex<Option<(Vec<u8>, Instant)>>>;
#[tauri::command]
fn workbench_public_stop(app: tauri::AppHandle) -> Result<(), String> {
    nai_workbench_host::public_output::stop(&audition_root(&app)?)
}
#[tauri::command]
async fn workbench_public_open(
    app: tauri::AppHandle,
    state: tauri::State<'_, PublicOutputOwner>,
) -> Result<Vec<u8>, String> {
    let root = audition_root(&app)?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut owner = state.0.try_lock().map_err(|_| "WB_BUSY")?;
        if owner
            .as_mut()
            .is_some_and(|o| o.poll().map_or(true, |s| s.terminal))
        {
            owner.take();
        }
        if owner.is_none() {
            *owner = Some(nai_workbench_host::public_output::OutputOwner::open_at(
                &nai_workbench_host::public_output::binary()?,
                &root,
                false,
            )?);
        }
        let bytes = nai_workbench_host::public_output::encoded(
            owner.as_ref().ok_or("WB_NOT_CONNECTED")?.current(),
        );
        if let Ok(mut snapshot) = state.1.try_lock() {
            *snapshot = Some((bytes.clone(), Instant::now()));
        }
        Ok(bytes)
    })
    .await
    .map_err(|_| "WB_WORKER_LOST".to_string())?
}
#[tauri::command]
async fn workbench_public_poll(
    state: tauri::State<'_, PublicOutputOwner>,
) -> Result<Vec<u8>, String> {
    let snapshot = state.1.try_lock().map_err(|_| "WB_BUSY")?;
    let (bytes, observed) = snapshot.as_ref().ok_or("WB_NOT_CONNECTED")?;
    if observed.elapsed() >= Duration::from_millis(500) {
        return Err("WB_DEADLINE_EXCEEDED".into());
    }
    Ok(bytes.clone())
}
#[tauri::command]
fn workbench_public_close(
    app: tauri::AppHandle,
    state: tauri::State<'_, PublicOutputOwner>,
) -> Result<(), String> {
    workbench_public_stop(app)?;
    state.0.try_lock().map_err(|_| "WB_BUSY")?.take();
    if let Ok(mut snapshot) = state.1.try_lock() {
        snapshot.take();
    }
    Ok(())
}
#[derive(Clone, Default)]
struct BusinessOwner(
    Arc<Mutex<Option<nai_workbench_host::foundation::Owner>>>,
    Arc<AtomicBool>,
);
#[derive(Clone, Default)]
struct CharacterOwner(Arc<Mutex<nai_media_core::character_workbench::CharacterWorkbench>>);
struct UserCommand(Arc<AtomicBool>);
impl Drop for UserCommand {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
fn audition_root(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|_| "WB_DATA_ROOT_REJECTED")?;
    #[cfg(debug_assertions)]
    let root = std::env::var_os("NAI_WORKBENCH_TEST_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or(root);
    if !root.is_absolute() {
        return Err("WB_DATA_ROOT_REJECTED".into());
    }
    Ok(root.join("foundation-v2"))
}
#[tauri::command]
fn workbench_stop_audition(app: tauri::AppHandle) -> Result<(), String> {
    nai_workbench_host::foundation::stop_audition(&audition_root(&app)?)
}
#[tauri::command]
fn workbench_audition_nonce(app: tauri::AppHandle) -> Result<Vec<u8>, String> {
    nai_workbench_host::foundation::audition_nonce(&audition_root(&app)?)
}
fn avatar_read_command(state: &BusinessOwner, operation: nai_contracts::workbench::foundation_command::Operation) -> Result<nai_contracts::workbench::FoundationResult, String> {
    use nai_contracts::workbench::{FoundationCommand, FoundationResponse, foundation_response::Outcome};
    use prost::Message;
    let command = FoundationCommand { idempotency_key: nai_workbench_host::foundation::random_id()?, operation: Some(operation) };
    let mut owner = state.0.try_lock().map_err(|_| "WB_BUSY")?;
    let response = owner.as_mut().ok_or("WB_NOT_CONNECTED")?.call(&command.encode_to_vec())?;
    match FoundationResponse::decode(response.as_slice()).map_err(|_| "WB_PROTOCOL_ERROR")?.outcome {
        Some(Outcome::Result(result)) => Ok(result),
        Some(Outcome::Problem(_)) => Err("AVATAR_WORKSPACE_DENIED".into()),
        _ => Err("WB_PROTOCOL_ERROR".into()),
    }
}

fn avatar_check_workspace(state: &BusinessOwner, workspace_id: &str) -> Result<(), String> {
    use nai_contracts::workbench::{FoundationBrowse, foundation_command::Operation};
    avatar_read_command(state, Operation::ListPreparation(FoundationBrowse { workspace_id: workspace_id.into(), limit: 1, ..Default::default() }))?;
    Ok(())
}

#[tauri::command]
async fn workbench_avatar_choose_and_admit(
    app: tauri::AppHandle,
    acknowledge_rights: bool,
    workspace_id: String,
    previews: tauri::State<'_, Arc<avatar_preview::PreviewSlot>>,
    owner: tauri::State<'_, BusinessOwner>,
) -> Result<Option<avatar::AdmissionMetadata>, String> {
    if !acknowledge_rights { return Err("AVATAR_RIGHTS_CONFIRMATION_REQUIRED".into()); }
    let root = audition_root(&app)?;
    let previews = previews.inner().clone();
    let owner = owner.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(windows)]
        {
            avatar_check_workspace(&owner, &workspace_id)?;
            let selected = rfd::FileDialog::new().set_title("选择数字人形象包文件夹").pick_folder();
            if selected.is_some() { avatar_check_workspace(&owner, &workspace_id)?; }
            avatar::admit_native_choice(root, selected, &workspace_id, &previews)
        }
        #[cfg(not(windows))]
        {
            let _ = (root, workspace_id, previews, owner);
            Err("AVATAR_NATIVE_IMPORT_UNAVAILABLE".into())
        }
    }).await.map_err(|_| "WB_WORKER_LOST".to_string())?
}
#[derive(serde::Serialize)]
#[serde(rename_all="camelCase")]
struct PhotoAdmission { sha256:String, width:u32, height:u32 }

#[tauri::command]
async fn workbench_avatar_choose_photo(app:tauri::AppHandle,workspace_id:String,acknowledge_rights:bool,previews:tauri::State<'_,Arc<avatar_preview::PreviewSlot>>,owner:tauri::State<'_,BusinessOwner>)->Result<Option<PhotoAdmission>,String> {
    if !acknowledge_rights {return Err("AVATAR_RIGHTS_CONFIRMATION_REQUIRED".into());}
    let root=audition_root(&app)?;let previews=previews.inner().clone();let owner=owner.inner().clone();
    tauri::async_runtime::spawn_blocking(move|| {
        #[cfg(windows)] {
            avatar_check_workspace(&owner,&workspace_id)?;
            let selected=rfd::FileDialog::new().set_title("选择形象照片").add_filter("JPEG 照片", &["jpg","jpeg"]).pick_file();
            let Some(path)=selected else {return Ok(None);};
            avatar_check_workspace(&owner,&workspace_id)?;
            let parent=path.parent().ok_or("PHOTO_PATH_REJECTED")?;
            let name=path.file_name().and_then(|x|x.to_str()).ok_or("PHOTO_PATH_REJECTED")?;
            let bytes=nai_avatar_files::SelectedDirectory::open(parent)?.read(name,nai_avatar_files::photo::MAX_PHOTO_BYTES)?;
            let meta=nai_avatar_files::photo::store_photo(&root,&bytes)?;
            previews.install_photo(&workspace_id,&meta.sha256,bytes).map_err(str::to_owned)?;
            Ok(Some(PhotoAdmission{sha256:meta.sha256,width:meta.width,height:meta.height}))
        }
        #[cfg(not(windows))] {let _=(root,previews,owner,workspace_id);Err("AVATAR_NATIVE_IMPORT_UNAVAILABLE".into())}
    }).await.map_err(|_|"WB_WORKER_LOST".to_string())?
}

#[tauri::command]
async fn workbench_avatar_photo_preview(app:tauri::AppHandle,workspace_id:String,object_id:String,previews:tauri::State<'_,Arc<avatar_preview::PreviewSlot>>,owner:tauri::State<'_,BusinessOwner>)->Result<PhotoAdmission,String> {
    let root=audition_root(&app)?;let previews=previews.inner().clone();let owner=owner.inner().clone();
    tauri::async_runtime::spawn_blocking(move||{
        use nai_contracts::workbench::{FoundationReadObject,foundation_command::Operation};
        owner.1.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire).map_err(|_|"WB_BUSY")?;let _reservation=UserCommand(owner.1.clone());
        previews.clear().map_err(str::to_owned)?;
        let result=avatar_read_command(&owner,Operation::ReadObject(FoundationReadObject{workspace_id:workspace_id.clone(),object_id,revision:0}))?;
        let doc:serde_json::Value=serde_json::from_str(&result.body).map_err(|_|"PHOTO_PROFILE_REJECTED")?;
        if doc["kind"]!="avatar_photo_draft" || doc["revoked"]!=false {return Err("PHOTO_PROFILE_REJECTED".into());}
        let digest=doc["photo_sha256"].as_str().ok_or("PHOTO_PROFILE_REJECTED")?;
        let (bytes,meta)=nai_avatar_files::photo::read_stored_photo(&root,digest)?;
        if doc["width"].as_u64()!=Some(u64::from(meta.width)) || doc["height"].as_u64()!=Some(u64::from(meta.height)) {return Err("PHOTO_PROFILE_REJECTED".into());}
        previews.install_photo(&workspace_id,digest,bytes).map_err(str::to_owned)?;
        Ok(PhotoAdmission{sha256:meta.sha256,width:meta.width,height:meta.height})
    }).await.map_err(|_|"WB_WORKER_LOST".to_string())?
}

#[derive(serde::Serialize)]
#[serde(rename_all="camelCase")]
struct VideoAdmission { sha256:String, duration_ms:u64, inspect_version:String, realistic_drive:String }

#[tauri::command]
async fn workbench_avatar_choose_video(app:tauri::AppHandle,workspace_id:String,acknowledge_rights:bool,previews:tauri::State<'_,Arc<avatar_preview::PreviewSlot>>,owner:tauri::State<'_,BusinessOwner>)->Result<Option<VideoAdmission>,String> {
    if !acknowledge_rights {return Err("AVATAR_RIGHTS_CONFIRMATION_REQUIRED".into());}
    let root=audition_root(&app)?;let previews=previews.inner().clone();let owner=owner.inner().clone();
    tauri::async_runtime::spawn_blocking(move|| {
        #[cfg(windows)] {
            avatar_check_workspace(&owner,&workspace_id)?;
            let selected=rfd::FileDialog::new().set_title("选择形象视频").add_filter("MP4 视频", &["mp4"]).pick_file();
            let Some(path)=selected else {return Ok(None);};
            avatar_check_workspace(&owner,&workspace_id)?;
            let parent=path.parent().ok_or("VIDEO_PATH_REJECTED")?;
            let name=path.file_name().and_then(|x|x.to_str()).ok_or("VIDEO_PATH_REJECTED")?;
            let bytes=nai_avatar_files::SelectedDirectory::open(parent)?.read(name,nai_avatar_files::video::MAX_VIDEO_BYTES)?;
            let meta=nai_avatar_files::video::store_video(&root,&bytes)?;
            previews.install_video(&workspace_id,&meta.sha256,bytes).map_err(str::to_owned)?;
            Ok(Some(VideoAdmission{sha256:meta.sha256,duration_ms:meta.duration_ms,inspect_version:meta.inspect_version.to_owned(),realistic_drive:"model_missing".into()}))
        }
        #[cfg(not(windows))] {let _=(root,previews,owner,workspace_id);Err("AVATAR_NATIVE_IMPORT_UNAVAILABLE".into())}
    }).await.map_err(|_|"WB_WORKER_LOST".to_string())?
}

#[tauri::command]
async fn workbench_avatar_video_preview(app:tauri::AppHandle,workspace_id:String,object_id:String,previews:tauri::State<'_,Arc<avatar_preview::PreviewSlot>>,owner:tauri::State<'_,BusinessOwner>)->Result<VideoAdmission,String> {
    let root=audition_root(&app)?;let previews=previews.inner().clone();let owner=owner.inner().clone();
    tauri::async_runtime::spawn_blocking(move||{
        use nai_contracts::workbench::{FoundationReadObject,foundation_command::Operation};
        owner.1.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire).map_err(|_|"WB_BUSY")?;let _reservation=UserCommand(owner.1.clone());
        previews.clear().map_err(str::to_owned)?;
        let result=avatar_read_command(&owner,Operation::ReadObject(FoundationReadObject{workspace_id:workspace_id.clone(),object_id,revision:0}))?;
        let doc:serde_json::Value=serde_json::from_str(&result.body).map_err(|_|"VIDEO_PROFILE_REJECTED")?;
        if doc["kind"]!="avatar_video_draft" || doc["revoked"]!=false {return Err("VIDEO_PROFILE_REJECTED".into());}
        let digest=doc["video_sha256"].as_str().ok_or("VIDEO_PROFILE_REJECTED")?;
        let (bytes,meta)=nai_avatar_files::video::read_stored_video(&root,digest)?;
        if doc["duration_ms"].as_u64()!=Some(meta.duration_ms)
            || doc["inspect_version"].as_str()!=Some(nai_avatar_files::video::INSPECT_VERSION)
            || doc["realistic_drive"].as_str()!=Some("model_missing") {return Err("VIDEO_PROFILE_REJECTED".into());}
        previews.install_video(&workspace_id,digest,bytes).map_err(str::to_owned)?;
        Ok(VideoAdmission{sha256:meta.sha256,duration_ms:meta.duration_ms,inspect_version:meta.inspect_version.to_owned(),realistic_drive:"model_missing".into()})
    }).await.map_err(|_|"WB_WORKER_LOST".to_string())?
}

#[tauri::command]
async fn workbench_avatar_stage_preview(app: tauri::AppHandle, workspace_id: String, object_id: String, previews: tauri::State<'_, Arc<avatar_preview::PreviewSlot>>, owner: tauri::State<'_, BusinessOwner>) -> Result<avatar::AdmissionMetadata, String> {
    let root = audition_root(&app)?;
    let previews = previews.inner().clone();
    let owner = owner.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        use nai_contracts::workbench::{FoundationReadObject, foundation_command::Operation};
        owner.1.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).map_err(|_| "WB_BUSY")?;
        let _reservation = UserCommand(owner.1.clone());
        previews.clear().map_err(str::to_owned)?;
        let result = avatar_read_command(&owner, Operation::ReadObject(FoundationReadObject { workspace_id: workspace_id.clone(), object_id, revision: 0 }))?;
        let doc: serde_json::Value = serde_json::from_str(&result.body).map_err(|_| "AVATAR_PROFILE_REJECTED")?;
        if doc["kind"] != "avatar_profile_draft" || doc["revoked"] != false { return Err("AVATAR_PROFILE_REJECTED".into()); }
        let package = doc["package_sha256"].as_str().ok_or("AVATAR_PROFILE_REJECTED")?;
        let rights = doc["license_sha256"].as_str().ok_or("AVATAR_PROFILE_REJECTED")?;
        avatar::stage_existing_preview(&root, &workspace_id, package, rights, &previews)
    }).await.map_err(|_| "WB_WORKER_LOST".to_string())?
}

#[derive(serde::Serialize)]
#[serde(rename_all="camelCase")]
struct CharacterWorkbenchView {
    state: i32,
    action_id: String,
    reason_code: String,
    public_backend: String,
    public_destination: String,
    playback_permit: bool,
    selected: bool,
}

fn character_err(err: nai_media_core::character_workbench::CharacterWorkbenchError) -> String {
    use nai_media_core::character_workbench::CharacterWorkbenchError::*;
    match err {
        UnsupportedAction => "UNSUPPORTED_ACTION".into(),
        Restore => "CHARACTER_RESTORE_REFUSED".into(),
        Unbound => "CHARACTER_UNBOUND".into(),
        Closed => "CHARACTER_CLOSED".into(),
        Identity => "CHARACTER_IDENTITY_REJECTED".into(),
        Session => "CHARACTER_SESSION_REJECTED".into(),
        Epoch => "CHARACTER_EPOCH_REJECTED".into(),
        Rights => "CHARACTER_RIGHTS_MISMATCH".into(),
        AlreadyBound => "CHARACTER_ALREADY_BOUND".into(),
        Host(_) => "CHARACTER_HOST_REJECTED".into(),
    }
}

fn character_pair(workspace_id: &str, avatar_id: &str, version: &str, package: &str, rights: &str) -> (nai_media_core::character_license::CharacterLicenseRecord, nai_contracts::v1::CharacterIdentity) {
    (
        nai_media_core::character_license::CharacterLicenseRecord {
            workspace_id: workspace_id.into(),
            package_sha256: package.into(),
            rights_sha256: rights.into(),
        },
        nai_contracts::v1::CharacterIdentity {
            workspace_id: workspace_id.into(),
            character_id: avatar_id.into(),
            version: version.into(),
            package_sha256: package.into(),
            rights_sha256: rights.into(),
        },
    )
}

fn encode_character_view(workbench: &nai_media_core::character_workbench::CharacterWorkbench, view: &nai_contracts::v1::CharacterRuntimeView) -> CharacterWorkbenchView {
    let output = workbench.public_output();
    CharacterWorkbenchView {
        state: view.state,
        action_id: view.action_id.clone(),
        reason_code: view.reason_code.clone(),
        public_backend: output.backend.into(),
        public_destination: output.destination.into(),
        playback_permit: output.playback_permit(),
        selected: workbench.selected().is_some_and(|selected| view.identity.as_ref() == Some(selected)),
    }
}

fn read_avatar_character_doc(owner: &BusinessOwner, workspace_id: String, object_id: String) -> Result<(String, String, String, String), String> {
    use nai_contracts::workbench::{FoundationReadObject, foundation_command::Operation};
    let result = avatar_read_command(owner, Operation::ReadObject(FoundationReadObject { workspace_id, object_id, revision: 0 }))?;
    let doc: serde_json::Value = serde_json::from_str(&result.body).map_err(|_| "AVATAR_PROFILE_REJECTED")?;
    if doc["kind"] != "avatar_profile_draft" || doc["revoked"] != false { return Err("AVATAR_PROFILE_REJECTED".into()); }
    Ok((
        doc["avatar_id"].as_str().ok_or("AVATAR_PROFILE_REJECTED")?.to_owned(),
        doc["package_version"].as_str().ok_or("AVATAR_PROFILE_REJECTED")?.to_owned(),
        doc["package_sha256"].as_str().ok_or("AVATAR_PROFILE_REJECTED")?.to_owned(),
        doc["license_sha256"].as_str().ok_or("AVATAR_PROFILE_REJECTED")?.to_owned(),
    ))
}

#[tauri::command]
async fn workbench_character_generate(
    workspace_id: String,
    object_id: String,
    characters: tauri::State<'_, CharacterOwner>,
    owner: tauri::State<'_, BusinessOwner>,
) -> Result<CharacterWorkbenchView, String> {
    let characters = characters.inner().clone();
    let owner = owner.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        owner.1.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).map_err(|_| "WB_BUSY")?;
        let _reservation = UserCommand(owner.1.clone());
        let (avatar_id, version, package, rights) = read_avatar_character_doc(&owner, workspace_id.clone(), object_id)?;
        let (license, identity) = character_pair(&workspace_id, &avatar_id, &version, &package, &rights);
        let mut workbench = characters.0.try_lock().map_err(|_| "WB_BUSY")?;
        let view = workbench.generate(license, identity, workspace_id).map_err(character_err)?;
        Ok(encode_character_view(&workbench, &view))
    }).await.map_err(|_| "WB_WORKER_LOST".to_string())?
}

#[tauri::command]
async fn workbench_character_select(
    workspace_id: String,
    object_id: String,
    characters: tauri::State<'_, CharacterOwner>,
    owner: tauri::State<'_, BusinessOwner>,
) -> Result<CharacterWorkbenchView, String> {
    let characters = characters.inner().clone();
    let owner = owner.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        owner.1.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).map_err(|_| "WB_BUSY")?;
        let _reservation = UserCommand(owner.1.clone());
        let (avatar_id, version, package, rights) = read_avatar_character_doc(&owner, workspace_id.clone(), object_id)?;
        let (license, identity) = character_pair(&workspace_id, &avatar_id, &version, &package, &rights);
        let mut workbench = characters.0.try_lock().map_err(|_| "WB_BUSY")?;
        let view = workbench.select(license, identity, workspace_id).map_err(character_err)?;
        Ok(encode_character_view(&workbench, &view))
    }).await.map_err(|_| "WB_WORKER_LOST".to_string())?
}

#[tauri::command]
async fn workbench_character_action(
    action_id: String,
    characters: tauri::State<'_, CharacterOwner>,
) -> Result<CharacterWorkbenchView, String> {
    let characters = characters.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut workbench = characters.0.try_lock().map_err(|_| "WB_BUSY")?;
        let view = workbench.preview(&action_id, 800).map_err(character_err)?;
        Ok(encode_character_view(&workbench, &view))
    }).await.map_err(|_| "WB_WORKER_LOST".to_string())?
}

#[tauri::command]
async fn workbench_character_revoke(
    characters: tauri::State<'_, CharacterOwner>,
) -> Result<CharacterWorkbenchView, String> {
    let characters = characters.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut workbench = characters.0.try_lock().map_err(|_| "WB_BUSY")?;
        let view = workbench.revoke().map_err(character_err)?;
        Ok(encode_character_view(&workbench, &view))
    }).await.map_err(|_| "WB_WORKER_LOST".to_string())?
}

#[tauri::command]
async fn workbench_open(
    app: tauri::AppHandle,
    state: tauri::State<'_, BusinessOwner>,
) -> Result<Vec<u8>, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|_| "WB_DATA_ROOT_REJECTED")?;
    #[cfg(debug_assertions)]
    let root = if let Some(path) = std::env::var_os("NAI_WORKBENCH_TEST_ROOT") {
        let path = std::path::PathBuf::from(path);
        if !path.is_absolute() {
            return Err("WB_DATA_ROOT_REJECTED".into());
        }
        path
    } else {
        root
    };
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut owner = state.0.try_lock().map_err(|_| "WB_BUSY")?;
        if owner.is_none() {
            *owner = Some(nai_workbench_host::foundation::Owner::open_at(
                &nai_workbench_host::foundation::binary()?,
                &root.join("operator-v43.json"),
                &root.join("foundation-v2"),
            )?);
        }
        let result = owner.as_mut().ok_or("WB_NOT_CONNECTED")?.list_workspaces();
        if result.is_err() {
            owner.take();
        }
        result
    })
    .await
    .map_err(|_| String::from("WB_WORKER_LOST"))?
}
#[tauri::command]
async fn foundation_command(
    app: tauri::AppHandle,
    public: tauri::State<'_, PublicOutputOwner>,
    state: tauri::State<'_, BusinessOwner>,
    previews: tauri::State<'_, Arc<avatar_preview::PreviewSlot>>,
    command: Vec<u8>,
) -> Result<Vec<u8>, String> {
    if command.len() > 16384 {
        return Err("WB_FRAME_LIMIT".into());
    }
    use prost::Message;
    if let Ok(c) = nai_contracts::workbench::FoundationCommand::decode(command.as_slice()) {
        use nai_contracts::workbench::foundation_command::Operation as C;
        if matches!(c.operation, Some(C::WorkspaceContext(_) | C::WorkspaceLifecycle(_) | C::PurgeWorkspace(_) | C::PutPreparation(_) | C::DeleteObject(_))) {
            previews.clear().map_err(str::to_owned)?;
        }
        if matches!(c.operation,Some(C::ControlLiveRun(ref x)) if x.action==nai_contracts::workbench::LiveRunAction::End as i32)
            || matches!(
                c.operation,
                Some(
                    C::WorkspaceContext(_)
                        | C::WorkspaceLifecycle(_)
                        | C::PurgeWorkspace(_)
                        | C::Grant(_)
                        | C::Stop(_)
                        | C::ControlLiveTakeover(_)
                )
            )
        {
            human::stop_owned(&app)?;
        }
    }
    let state = state.inner().clone();
    let public = public.inner().clone();
    let root = audition_root(&app)?;
    state
        .1
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| "WB_BUSY")?;
    let reservation = UserCommand(state.1.clone());
    tauri::async_runtime::spawn_blocking(move || {
        let _reservation = reservation;
        let until = Instant::now() + Duration::from_millis(250);
        loop {
            // Wait only before dispatch; a command or paid call is never replayed.
            let mut output = if nai_workbench_host::foundation::needs_public_output(&command) {
                match public.0.try_lock() {
                    Ok(v) => Some(v),
                    Err(_) => {
                        if Instant::now() >= until {
                            return Err("WB_BUSY".into());
                        }
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                }
            } else {
                None
            };
            let mut owner = match state.0.try_lock() {
                Ok(v) => v,
                Err(_) => {
                    drop(output);
                    if Instant::now() >= until {
                        return Err("WB_BUSY".into());
                    }
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
            };
            let business = owner.as_mut().ok_or("WB_NOT_CONNECTED")?;
            let result = if let Some(output) = output.as_mut() {
                business.call_with_public_output(&command, output.as_mut(), &root)
            } else {
                business.call(&command)
            };
            if result.is_err() {
                owner.take();
            }
            return result;
        }
    })
    .await
    .map_err(|_| String::from("WB_WORKER_LOST"))?
}
#[tauri::command]
fn workbench_close(
    app: tauri::AppHandle,
    state: tauri::State<'_, BusinessOwner>,
) -> Result<(), String> {
    human::stop_owned(&app)?;
    workbench_stop_audition(app)?;
    state.0.try_lock().map_err(|_| "WB_BUSY")?.take();
    Ok(())
}
#[tauri::command]
async fn workbench_inspect(
    app: tauri::AppHandle,
    presentation: i32,
    prepare: bool,
) -> Result<Vec<u8>, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|_| "WB_DATA_ROOT_REJECTED")?
        .join("bootstrap-v43");
    let engine = nai_workbench_host::binary()?;
    tauri::async_runtime::spawn_blocking(move || {
        nai_workbench_host::inspect_at(&engine, &root, presentation, prepare)
    })
    .await
    .map_err(|_| "WB_WORKER_LOST")?
}
#[tauri::command]
fn frontend_ready(ready: tauri::State<'_, Arc<AtomicBool>>) {
    ready.store(true, Ordering::Release);
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let smoke = args == ["--smoke"];
    if !args.is_empty() && !smoke {
        std::process::exit(2);
    }
    let ready = Arc::new(AtomicBool::new(false));
    let halt = Arc::new(AtomicBool::new(false));
    let exit_halt = halt.clone();
    tauri::Builder::default().manage(ready.clone()).manage(MicService::default()).manage(BusinessOwner::default()).manage(PublicOutputOwner::default()).manage(CharacterOwner::default()).manage(Arc::new(avatar_preview::PreviewSlot::default())).register_uri_scheme_protocol("avatarpreview", |ctx, request| {
        let state = ctx.app_handle().state::<Arc<avatar_preview::PreviewSlot>>();
        avatar_preview::response(state.inner(), ctx.webview_label(), request.method().as_str(), request.uri().path())
    }).invoke_handler(tauri::generate_handler![workbench_avatar_choose_and_admit,workbench_avatar_stage_preview,workbench_avatar_choose_photo,workbench_avatar_photo_preview,workbench_avatar_choose_video,workbench_avatar_video_preview,workbench_character_generate,workbench_character_action,workbench_character_select,workbench_character_revoke,workbench_all_mute,workbench_mic_stop,workbench_mic_devices,workbench_mic_poll,workbench_human_start,workbench_human_resume,workbench_public_open,workbench_public_poll,workbench_public_stop,workbench_public_close,workbench_obs_observe,workbench_stop_audition,workbench_audition_nonce,workbench_inspect,frontend_ready,workbench_open,foundation_command,workbench_close]).setup(move|app|{
  human::supervise(app.handle().clone(),halt.clone());
  let root=audition_root(app.handle())?;let output=app.state::<PublicOutputOwner>().inner().clone();let business=app.state::<BusinessOwner>().inner().clone();let output_state=output.1.clone();
  std::thread::spawn(move||{while !halt.load(Ordering::Acquire){if business.1.load(Ordering::Acquire){std::thread::sleep(Duration::from_millis(10));continue;}let next=Instant::now()+Duration::from_millis(100);
    if let Ok(mut public)=output.0.try_lock() && let Some(output)=public.as_mut(){
      let result=if let Ok(mut owner)=business.0.try_lock(){if let Some(owner)=owner.as_mut(){owner.renew_public_output(output,&root)}else{output.poll()}}else{output.poll()};
      match result{Ok(status)=>{if let Ok(mut snapshot)=output_state.try_lock(){*snapshot=Some((nai_workbench_host::public_output::encoded(status),Instant::now()));}},Err(_)=>{public.take();if let Ok(mut snapshot)=output_state.try_lock(){snapshot.take();}}}
    }
    std::thread::sleep(next.saturating_duration_since(Instant::now()));
  }});
  if smoke{let handle=app.handle().clone();std::thread::spawn(move||{let deadline=Instant::now()+Duration::from_secs(20);while Instant::now()<deadline{if ready.load(Ordering::Acquire){println!("WORKBENCH_PREPARATION_READY: authenticated owner protocol=2 live_ready=false");handle.exit(0);return;}std::thread::sleep(Duration::from_millis(25));}eprintln!("WORKBENCH_PREPARATION_TIMEOUT");handle.exit(1);});}Ok(())
 }).build(tauri::generate_context!()).expect("workbench desktop failed").run(move|app,event|{if matches!(event,tauri::RunEvent::Exit){exit_halt.store(true,Ordering::Release);let _=human::stop_owned(app);let _=workbench_stop_audition(app.clone());let _=workbench_public_stop(app.clone());let public=app.state::<PublicOutputOwner>();if let Ok(mut owner)=public.0.try_lock(){owner.take();}let owner=app.state::<BusinessOwner>();if let Ok(mut state)=owner.0.try_lock(){state.take();}}});
}
