#include <cstdlib>
#include <iostream>

namespace {

int present(const char *name) {
    const char *value = std::getenv(name);
    return value != nullptr && value[0] != '\0';
}

}  // namespace

int main() {
    int test_tmpdir = present("TEST_TMPDIR");
    int test_srcdir = present("TEST_SRCDIR");
    int test_workspace = present("TEST_WORKSPACE");
    int xml_output = present("XML_OUTPUT_FILE");
    int runfiles_manifest = present("RUNFILES_MANIFEST_FILE");

    std::cout << "test-tmpdir=" << test_tmpdir << "\n";
    std::cout << "test-srcdir=" << test_srcdir << "\n";
    std::cout << "test-workspace=" << test_workspace << "\n";
    std::cout << "xml-output=" << xml_output << "\n";
    std::cout << "runfiles-manifest=" << runfiles_manifest << "\n";

    return test_tmpdir && test_srcdir && test_workspace ? 0 : 1;
}
