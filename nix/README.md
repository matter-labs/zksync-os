# Reproducible builds with nix

from the checked-out project root, run on a `x86_64` machine:

```shell
docker run --privileged -it -v .:/mnt nixos/nix:2.18.1
```

then run inside the container shell:

```shell
echo 'experimental-features = nix-command flakes' >> /etc/nix/nix.conf
echo 'sandbox = true' >> /etc/nix/nix.conf
cd /mnt
nix build -L
# copies from the container /nix storage into the project
cp -avr result ./build-artifacts
```

The directory `build-artifacts` should contain:

```shell
$ ls build-artifacts
app.bin  app.elf  app.text
```

Running it on a macOS darwin machine is possible, but will most likely give different results.
