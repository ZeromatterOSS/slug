#include <stdio.h>
#include "lib/math.h"

int main() {
    int failures = 0;

    if (add(2, 3) != 5) {
        printf("FAIL: add(2, 3) expected 5, got %d\n", add(2, 3));
        failures++;
    }

    if (multiply(4, 5) != 20) {
        printf("FAIL: multiply(4, 5) expected 20, got %d\n", multiply(4, 5));
        failures++;
    }

    if (add(0, 0) != 0) {
        printf("FAIL: add(0, 0) expected 0\n");
        failures++;
    }

    if (failures == 0) {
        printf("All tests passed!\n");
    }
    return failures;
}
