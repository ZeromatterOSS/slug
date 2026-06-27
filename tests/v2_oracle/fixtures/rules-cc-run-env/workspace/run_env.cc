#include <cstdlib>
#include <iostream>

namespace {

int present(const char *name) {
    const char *value = std::getenv(name);
    return value != nullptr && value[0] != '\0';
}

}  // namespace

int main() {
    std::cout << "build-workspace-directory=" << present("BUILD_WORKSPACE_DIRECTORY") << "\n";
    std::cout << "build-working-directory=" << present("BUILD_WORKING_DIRECTORY") << "\n";
    std::cout << "runfiles-manifest=" << present("RUNFILES_MANIFEST_FILE") << "\n";
    std::cout << "runfiles-dir=" << present("RUNFILES_DIR") << "\n";
    return 0;
}
