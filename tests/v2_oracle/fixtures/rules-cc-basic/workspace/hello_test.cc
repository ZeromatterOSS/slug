#include "hello.h"

#include <cstring>

int main() {
    return std::strcmp(hello_message(), "hello rules_cc") == 0 ? 0 : 1;
}