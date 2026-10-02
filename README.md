# terrain

Move 3D objects on screen with your real hands. Rust + Bevy + MediaPipe hand models (ONNX).

## Run

```sh
cargo run -p terrain-app                          # bundled demo replay, looped
cargo run -p terrain-app -- --replay <file.json>  # your own recorded HandFrames
```

Live webcam tracking is not wired in yet; see issue #1 for the plan.

## Test

```sh
cargo test --workspace
```

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
