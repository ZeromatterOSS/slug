#include <stdio.h>
#include "lib/math.h"
#include "lib/platform.h"

int main() {
    printf("Platform: %s\n", get_platform_name());
    printf("3 + 4 = %d\n", add(3, 4));
    printf("3 * 4 = %d\n", multiply(3, 4));
    return 0;
}
