STATE = "V1_SENTINEL"
IS_V1 = STATE[1] == "1"
print("M1_INFLIGHT_SOURCE_" + ("V1" if IS_V1 else "V2"))
ROOT_SRCS = ["before.txt" if IS_V1 else "after.txt", "//b:b"]
