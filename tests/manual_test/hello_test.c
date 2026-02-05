// Simple test for hello()
extern int hello(void);

int main(void) {
    int result = hello();
    // Simple assertion - return 0 on success, non-zero on failure
    if (result == 42) {
        return 0;  // Test passed
    }
    return 1;  // Test failed
}
