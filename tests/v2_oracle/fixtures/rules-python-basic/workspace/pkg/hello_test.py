from pkg.hello import message

if message() != "hello rules_python":
    raise SystemExit(1)