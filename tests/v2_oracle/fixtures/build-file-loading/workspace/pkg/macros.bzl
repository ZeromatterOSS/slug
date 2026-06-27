def make_export(name, src):
    native.exports_files([src])
    native.filegroup(name = name, srcs = [src])