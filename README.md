# Binary Ninja Minidump Loader

A Minidump loader plugin for Binary Ninja.

![Screenshot of Binary Ninja using the "Minidump" Binary View, with a minidump loaded and the virtual addresses of the memory segments of the minidump showing in the Memory Map window](images/loaded-minidump-screenshot-border.png)

## Building and Installing

This plugin currently needs to be built from source, then copied into your user plugin folder.

```
cargo build --release
cp target/release/libminidump_bn.so ~/.binaryninja/plugins/
```

The code in this plugin targets the `dev` branch of the [Binary Ninja Rust API](https://github.com/Vector35/binaryninja-api/tree/dev/rust).
