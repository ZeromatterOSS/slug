def _identity(value):
    return value

def _concat(left, right):
    return left + right

def _pattern(value):
    return value + "/**/*.txt"

def _project(name, ok):
    native.filegroup(name = name + ("_pass" if ok else "_fail"))

def dynamic_projection(name, build_value):
    returned = _identity(build_value)
    joined = _concat(returned, "\377")
    stored = ["\351", returned]
    by_byte_key = {
        "\303\251": "two",
        "\351": "one",
    }
    pattern = _identity(_pattern(returned))
    ok = (
        returned == "\303\251" and
        returned != "\351" and
        len(returned) == 2 and
        joined == "\303\251\377" and
        returned * 2 == "\303\251\303\251" and
        returned[0] == "\303" and
        returned[1:] == "\251" and
        returned in stored and
        "\351" in stored and
        by_byte_key[returned] == "two" and
        by_byte_key["\351"] == "one" and
        len(by_byte_key) == 2 and
        pattern == "\303\251/**/*.txt" and
        pattern[:2] == returned
    )
    _project(name, ok)
    return returned

def static_projection(name, build_literal, macro_return):
    single = 'é'
    double = "é"
    raw = r"é"
    triple = """é"""
    two_octal = "\303\251"
    one_octal = "\351"

    non_bmp = "😀"
    ordered = sorted([one_octal, double, "\251"])
    ok = (
        single == double and
        raw == double and
        triple == double and
        double == two_octal and
        double != one_octal and
        build_literal == two_octal and
        macro_return == two_octal and
        r"\303\251" == "\\303\\251" and
        len("") == 0 and
        len("\0") == 1 and
        "\0" == "\000" and
        len("\377") == 1 and
        "\3777" == "\377" + "7" and
        "\378" == "\37" + "8" and
        len("\3777") == 2 and
        len("\378") == 2 and
        len(non_bmp) == 4 and
        non_bmp == "\360\237\230\200" and
        "\251" < double and
        double < one_octal and
        ordered == ["\251", double, one_octal]
    )
    _project(name, ok)
