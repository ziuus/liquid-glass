# Known Limitations

- **Backdrop Acquisition Hack**: Background pixels are currently acquired using an out-of-process CLI tool (`grim`) because standard `wlr-layer-shell` does not pass down background samples. This must be replaced with `wlr-screencopy-unstable-v1` natively within the Wayland client loop.
- **Overlapping Glass Feedback**: Refraction of intersecting glass panels currently does not correctly aggregate because all panels sample from a single static backdrop texture per frame rather than intermediate composition buffers.
- **Interactivity & Deformation**: Interactive liquid ripples based on pointer velocity are not yet connected to the uniform buffers.
- **Platform Coupling**: True cross-platform operation (macOS/Windows) requires decoupling of Wayland setup routines in example drivers.
- **Frame Timing Profiling**: CPU frame time is distinct from GPU execution time; wgpu timestamp queries need to be integrated for accurate uncapped benchmarking.
