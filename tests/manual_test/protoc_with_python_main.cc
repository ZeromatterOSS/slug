// Protocol Buffers - protoc with Python generator only
// Minimal protoc that includes the Python code generator.
// Used by py_proto_library in the Kuro test environment.

#include "absl/log/initialize.h"
#include "google/protobuf/compiler/command_line_interface.h"
#include "google/protobuf/compiler/python/generator.h"
#include "google/protobuf/compiler/python/pyi_generator.h"

// Must be included last.
#include "google/protobuf/port_def.inc"

namespace google {
namespace protobuf {
namespace compiler {

int ProtocWithPythonMain(int argc, char* argv[]) {
  absl::InitializeLog();

  CommandLineInterface cli;
  cli.AllowPlugins("protoc-");

  // Register Python generator for --python_out
  python::Generator python_generator;
  cli.RegisterGenerator("--python_out", "--python_opt",
                        &python_generator,
                        "Generate Python source file.");

  // Register pyi generator for --pyi_out
  python::PyiGenerator pyi_generator;
  cli.RegisterGenerator("--pyi_out", &pyi_generator,
                        "Generate Python .pyi stub file.");

  return cli.Run(argc, argv);
}

}  // namespace compiler
}  // namespace protobuf
}  // namespace google

int main(int argc, char* argv[]) {
  return google::protobuf::compiler::ProtocWithPythonMain(argc, argv);
}

#include "google/protobuf/port_undef.inc"
