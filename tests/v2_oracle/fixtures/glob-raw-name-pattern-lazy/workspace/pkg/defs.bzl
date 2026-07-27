def _state(actual, present, absent):
    if actual == present:
        return "present"
    if actual == absent:
        return "absent"
    return "mismatch"
def dynamic_pattern():
    return "\303" + "\251*.txt"

def raw_name_targets():
    utf8_name = "\303\251.txt"
    raw_name = "\351.txt"

    all_txt = native.glob(["*.txt"], allow_empty = True)
    utf8_literal = native.glob(["é*.txt"], allow_empty = True)
    raw_octal = native.glob(["\351*.txt"], allow_empty = True)
    loaded_dynamic = native.glob([dynamic_pattern()], allow_empty = True)

    if all_txt == [utf8_name, raw_name]:
        all_state = "c3a9_before_e9"
    elif all_txt == [utf8_name]:
        all_state = "c3a9_only"
    else:
        all_state = "mismatch"

    raw_state = _state(raw_octal, [raw_name], [])
    native.filegroup(name = "all_txt_" + all_state)
    native.filegroup(name = "utf8_literal_c3a9_only" if utf8_literal == [utf8_name] else "utf8_literal_mismatch")
    native.filegroup(name = "octal_e9_present_only" if raw_state == "present" else ("octal_e9_absent" if raw_state == "absent" else "octal_e9_mismatch"))
    native.filegroup(name = "loaded_dynamic_c3a9_only" if loaded_dynamic == [utf8_name] else "loaded_dynamic_mismatch")
