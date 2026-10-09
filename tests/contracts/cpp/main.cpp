#include <cstdlib>
#include <fstream>
#include <iostream>
#include <string>
#include "core/base.pb.h"
#include "core/capability.pb.h"
#include "core/envelope.pb.h"
#include "core/error.pb.h"
#include "domain/agent.pb.h"
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
  std::cout << "C++ cross-language roundtrip passed (13 message types)\n";
}
