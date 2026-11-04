#include <cstdlib>
#include <cstdint>
#include <fstream>
#include <iostream>
#include <string>
#include "core/base.pb.h"
#include "core/capability.pb.h"
#include "core/envelope.pb.h"
#include "core/error.pb.h"
#include "domain/agent.pb.h"
#include "domain/control.pb.h"
#include "domain/media.pb.h"
#include "domain/platform.pb.h"

template<class M> M read(const std::string& directory, const std::string& name) {
  std::ifstream input(directory + "/" + name + ".bin", std::ios::binary);
  M message;
  if (!input || !message.ParseFromIstream(&input)) { std::cerr << "decode failed: " << name; std::exit(1); }
  return message;
}
template<class M> void transfer(const std::string& from, const std::string& to, const std::string& name) {
  M message = read<M>(from, name);
  std::ofstream output(to + "/" + name + ".bin", std::ios::binary);
  if (!output || !message.SerializeToOstream(&output)) { std::cerr << "encode failed: " << name; std::exit(1); }
}
int main(int argc, char** argv) {
  if (argc != 3) return 1;
  auto envelope = read<nai::v1::Envelope>(argv[1], "envelope");
  if (envelope.generation_epoch() != 9007199254740993ULL || envelope.sent_at_utc_ms() != 18446744073709551615ULL) return 1;
  auto permissions = read<nai::v1::PermissionSnapshot>(argv[1], "permissions");
  if (permissions.capabilities().at("unknown-test") != 77) return 1;
  transfer<nai::v1::Envelope>(argv[1], argv[2], "envelope");
  transfer<nai::v1::PerformanceCue>(argv[1], argv[2], "cue");
  transfer<nai::v1::AudioBlockMetadata>(argv[1], argv[2], "audio");
  transfer<nai::v1::PermissionSnapshot>(argv[1], argv[2], "permissions");
  transfer<nai::v1::ContractError>(argv[1], argv[2], "error");
  transfer<nai::v1::CapabilityResult>(argv[1], argv[2], "stream");
  transfer<nai::v1::AuthoritativeFact>(argv[1], argv[2], "fact");
  transfer<nai::v1::PrepareReport>(argv[1], argv[2], "prepare");
  auto ledger = read<nai::v1::UsageLedgerEntry>(argv[1], "ledger");
  if (ledger.estimate().cost_minor() != 18446744073709551615ULL || ledger.estimate().units().at("tokens") != 9007199254740993ULL || ledger.has_actual() || ledger.state() != nai::v1::NEEDS_RECONCILIATION) return 1;
  transfer<nai::v1::UsageLedgerEntry>(argv[1], argv[2], "ledger");
  auto platform = read<nai::v1::PlatformEvent>(argv[1], "platform");
  if (!platform.has_generation_epoch() || platform.generation_epoch() != 9007199254740993ULL || platform.connection_generation() != 18446744073709551615ULL) return 1;
  transfer<nai::v1::PlatformEvent>(argv[1], argv[2], "platform");
  auto watchdog_command = read<nai::v1::OutputWatchdogCommand>(argv[1], "watchdog_command");
  if (watchdog_command.seq() != 18446744073709551615ULL || !watchdog_command.has_arm() || watchdog_command.arm().generation_epoch() != 9007199254740993ULL) return 1;
  transfer<nai::v1::OutputWatchdogCommand>(argv[1], argv[2], "watchdog_command");
  auto watchdog_status = read<nai::v1::OutputWatchdogStatus>(argv[1], "watchdog_status");
  if (watchdog_status.epoch_floor() != 9007199254740993ULL || watchdog_status.output_available() || watchdog_status.backend_error().code() != nai::v1::CAPABILITY_NOT_READY) return 1;
  transfer<nai::v1::OutputWatchdogStatus>(argv[1], argv[2], "watchdog_status");
  auto slot = read<nai::v1::FrameSlotEvent>(argv[1], "frame_slot");
  if (!slot.has_committed() || slot.committed().ticket().generation_epoch() != 9007199254740993ULL || slot.committed().ticket().slot_generation() != 18446744073709551615ULL || slot.committed().ticket().frame().pts() != 9223372036854775807LL || slot.committed().ticket().frame().frame_seq() != 18446744073709551615ULL || slot.committed().byte_length() != 8) return 1;
  transfer<nai::v1::FrameSlotEvent>(argv[1], argv[2], "frame_slot");
  auto device = read<nai::v1::NativeDeviceSmokeReport>(argv[1], "device_smoke");
  if (device.callbacks() != 9007199254740993ULL || device.copied_interleaved_samples() != 18446744073709551615ULL || device.max_callback_frames() != 1056 || !device.audio_opened() || !device.window_opened() || device.success() || device.actual_local_playback_completed() || device.error_code() != "DEVICE_TIMING_UNAVAILABLE") return 1;
  transfer<nai::v1::NativeDeviceSmokeReport>(argv[1], argv[2], "device_smoke");
  auto local_media = read<nai::v1::LocalPlaybackMedia>(argv[1], "local_media");
  if (local_media.generation_epoch() != 9007199254740993ULL || local_media.sample_count() != 18446744073709551615ULL) return 1;
  if (local_media.frame_sha256_size()!=2 || local_media.frame_sha256(1)!=std::string(64,'f') || local_media.standby_sha256()!=std::string(64,'a')) return 1;
  transfer<nai::v1::LocalPlaybackMedia>(argv[1], argv[2], "local_media");
  auto local_grant = read<nai::v1::LocalPlaybackGrant>(argv[1], "local_grant");
  if (local_grant.seq() != 18446744073709551615ULL || local_grant.approval().generation_epoch() != 9007199254740993ULL || local_grant.media_sha256() != std::string(64, 'd')) return 1;
  transfer<nai::v1::LocalPlaybackGrant>(argv[1], argv[2], "local_grant");
  auto local_status = read<nai::v1::LocalPlaybackStatus>(argv[1], "local_status");
  if (local_status.has_copied_samples() || !local_status.has_window_updates() || local_status.window_updates() != 0 || local_status.actual_local_playback_completed()) return 1;
  if (local_status.audio_stream_open_count()!=1 || local_status.standby_sha256()!=std::string(64,'a')) return 1;
  if (!local_status.has_video_frame_index() || local_status.video_frame_index()!=0 || !local_status.has_video_device_samples() || local_status.video_device_samples()!=UINT64_MAX || local_status.video_tail_submitted()) return 1;
  transfer<nai::v1::LocalPlaybackStatus>(argv[1], argv[2], "local_status");
  auto probe = read<nai::v1::ProviderProbeReport>(argv[1], "provider_probe");
  if (probe.has_sample_count() || !probe.has_elapsed_ms() || probe.elapsed_ms()!=9007199254740993ULL || probe.invoice_known() || probe.playback_ack() || probe.usage().provider_ref()!="speech-backup") return 1;
  transfer<nai::v1::ProviderProbeReport>(argv[1], argv[2], "provider_probe");
  auto credentials = read<nai::v1::ProviderCredentialStatus>(argv[1], "provider_credentials");
  if (credentials.version()!=1 || credentials.saved_provider_ids_size()!=2 || credentials.saved_provider_ids(0)!="agnes-primary") return 1;
  transfer<nai::v1::ProviderCredentialStatus>(argv[1], argv[2], "provider_credentials");
  auto descriptor = read<nai::v1::LocalOutputCommand>(argv[1], "output_descriptor");
  if (!descriptor.has_descriptor_path() || descriptor.descriptor_path() != "C:/测试/media.pb") return 1;
  auto grant = read<nai::v1::LocalOutputCommand>(argv[1], "output_grant");
  if (!grant.has_grant() || grant.grant().seq() != 18446744073709551615ULL || grant.grant().lease().generation_epoch() != 9007199254740993ULL) return 1;
  auto cancel = read<nai::v1::LocalOutputCommand>(argv[1], "output_cancel");
  if (!cancel.has_cancel_cue() || !cancel.cancel_cue()) return 1;
  auto shutdown = read<nai::v1::LocalOutputCommand>(argv[1], "output_shutdown_false");
  if (!shutdown.has_shutdown() || shutdown.shutdown()) return 1;
  auto neutral=read<nai::v1::LocalStandbyMedia>(argv[1],"standby_media");
  if(neutral.session_id()!="静默会话" || neutral.generation_epoch()!=9007199254740993ULL || neutral.width()!=640 || neutral.height()!=360 || neutral.standby_sha256()!=std::string(64,'a'))return 1;
  transfer<nai::v1::LocalStandbyMedia>(argv[1],argv[2],"standby_media");
  auto warmup=read<nai::v1::LocalOutputCommand>(argv[1],"output_warmup");
  if(!warmup.has_warmup() || warmup.warmup().session_id()!="静默会话" || warmup.warmup().generation_epoch()!=9007199254740993ULL || warmup.warmup().remaining_budget_ms()!=4000 || warmup.warmup().descriptor_sha256()!=std::string(64,'d') || warmup.warmup().descriptor_path()!="C:/测试/standby.pb")return 1;
  for (const auto& name : {"output_descriptor", "output_grant", "output_cancel", "output_shutdown_false", "output_warmup"})
    transfer<nai::v1::LocalOutputCommand>(argv[1], argv[2], name);
  std::cout << "C++ cross-language roundtrip passed (25 fixtures / 22 message types)\n";
}
