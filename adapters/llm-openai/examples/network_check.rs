//! Fixed, unauthenticated HEAD diagnostics. Never submits a synthesis request.
use std::time::{Duration, Instant};
fn main() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("diagnostic runtime");
    runtime.block_on(async{for direct in [false,true]{let started=Instant::now();let builder=reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).retry(reqwest::retry::never()).connect_timeout(Duration::from_secs(3)).timeout(Duration::from_secs(10));let builder=if direct{builder.no_proxy()}else{builder};let outcome=match builder.build(){Ok(client)=>match client.head("https://api.siliconflow.cn/v1/models").send().await{Ok(r)=>serde_json::json!({"http_status":r.status().as_u16()}),Err(e)=>serde_json::json!({"failure":if e.is_timeout(){"TIMEOUT"}else if e.is_connect(){"CONNECTION_FAILED"}else{"NETWORK_FAILED"}})},Err(_)=>serde_json::json!({"failure":"CLIENT_UNAVAILABLE"})};println!("{}",serde_json::json!({"route":if direct{"direct"}else{"system_proxy"},"elapsed_ms":started.elapsed().as_millis(),"method":"HEAD","authenticated":false,"synthesis":false,"outcome":outcome}));}});
}
