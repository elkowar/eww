#How to install eww?
You have to build the binary using the commands given below-

```
$ git clone https://github.com/Elkowar/eww.git
$ cd eww
$ cargo build --release
```

After the build is successful you have to copy the built binary from ./target/release to anywhere in \$PATH (example - ~/.local/bin).
