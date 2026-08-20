## 0.3.1

* Added configurable OpenGL background color support with the `bgColor` option.
* Improved visibility of low-value point cloud data by adjusting the display value range.

## 0.3.0

* Added Android platform support with OpenGL ES rendering.
* Added pinch zoom gesture support for mobile viewers.
* Added optional dual joystick controls for XY movement and camera rotation.
* Added the `enableJoystick` option for controlling joystick visibility and interaction mode.
* Added variable joystick input speed based on thumb movement distance.
* Changed camera pan movement to use fixed world XY coordinates.
* Improved OpenGL viewport initialization and resize handling.
* Refactored Rust rendering modules and Flutter native bindings.
* Improved Android surface rendering, shader selection, and coordinate handling.

## 0.2.0

* Added optional mouse coordinate display to `PointGlassOpenGLViewer`.
* Added the `enableMouseCoordinate` option for controlling coordinate visibility.
* Improved pointer interaction and label repaint behavior.

## 0.1.0

* Initial release.
* Added Rust-based OpenGL offscreen rendering for native 3D visualization.
* Added Windows/Linux rendering support.
* Added FFI-based Point Cloud rendering with VBO support.
* Added rendering support for points, lines, polygons, axes, grids, and labels.
* Added 3D camera controls with orbit, pan, and roll interactions.
* Added depth-based color mapping and display control APIs.
* Added declarative viewer models and external controller support.
