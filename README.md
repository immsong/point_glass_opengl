# point_glass_opengl

A Flutter package for high-performance 3D point cloud visualization using native Rust, OpenGL, and OpenGL ES rendering.

`point_glass_opengl` provides a native rendering pipeline for visualizing point cloud data in Flutter applications on Windows, Linux, and Android. It supports interactive 3D camera controls, grid and axis overlays, label projection, mouse coordinate display, optional joystick controls, configurable background colors, X-axis viewer flipping, per-batch rendering settings, and real-time display parameter updates.

## Features

* **Native 3D Rendering**: Rust-based OpenGL and OpenGL ES rendering core for high-performance 3D visualization.
* **Point Cloud Rendering**: FFI-based point cloud rendering pipeline with VBO support.
* **Per-batch Rendering**: Independent color maps, point sizes, opacity, and value ranges for multiple point cloud batches in a shared buffer.
* **Cross-platform Support**: Native rendering support for Windows, Linux, and Android.
* **3D Camera Controls**: Built-in orbit, pan, roll, and zoom controls.
* **Joystick Controls**: Optional dual joystick controls for XY movement and camera rotation.
* **Visualization Overlays**: Grid, axis, and 3D-to-2D label projection support.
* **Mouse Coordinate Display**: Optional cursor overlay showing the corresponding X/Y position on the ground plane.
* **Depth-based Color Mapping**: Real-time point cloud color mapping using a value range.
* **Display Controls**: Runtime control of point size, alpha, value range, color mode, and background color.
* **X-axis Flip**: Toggle the rendered scene orientation along the X axis via the controller, without changing underlying point data.
* **Declarative Flutter API**: Flutter-friendly viewer models and external controller support.

## Platform Support

| Platform | Status            |
| -------- | ----------------- |
| Windows  | Supported         |
| Linux    | Supported         |
| macOS    | Not supported yet |
| Android  | Supported         |
| iOS      | Not supported yet |
| Web      | Not supported     |

## Installation

Add this to your package's `pubspec.yaml` file:

```yaml
dependencies:
  point_glass_opengl: ^0.4.1
```

Then run:

```bash
flutter pub get
```

## Basic Usage

```dart
import 'package:flutter/material.dart';
import 'package:point_glass_opengl/point_glass_opengl.dart';

void main() {
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return const MaterialApp(
      home: Scaffold(
        body: PointGlassExample(),
      ),
    );
  }
}

class PointGlassExample extends StatefulWidget {
  const PointGlassExample({super.key});

  @override
  State<PointGlassExample> createState() => _PointGlassExampleState();
}

class _PointGlassExampleState extends State<PointGlassExample> {
  final PointGlassOpenGLController _controller = PointGlassOpenGLController();

  @override
  Widget build(BuildContext context) {
    return PointGlassOpenGLViewer(
      controller: _controller,
      bgColor: const Color(0xFF1A1A1A),
      grid: PointGlassOpenGLGrid(),
      axis: PointGlassOpenGLAxis(),
    );
  }
}
```

## Updating Point Cloud Data

Use `PointGlassOpenGLController` to update point cloud data at runtime.

Point cloud data should be passed as a `Float32List`.

Each point uses the following format:

```text
[X, Y, Z, Value]
```

Example:

```dart
import 'dart:typed_data';

final pointData = Float32List.fromList([
  // x, y, z, value
  0.0, 0.0, 0.0, 0.0,
  1.0, 0.0, 0.5, 0.5,
  0.0, 1.0, 1.0, 1.0,
]);

_controller.setPoints(pointData);
```

The existing `setPoints()` API continues to render the point cloud using a single set of display parameters.

## Per-batch Point Cloud Rendering

Version 0.4.1 introduces `PointGlassOpenGLPointBatch` for rendering different portions of the same point cloud with independent display settings. Batches share the underlying point buffer and are drawn with their own color map, point size, opacity, and value range.

Use `setPointBatches()` to submit point data and its batch configuration together:

```dart
import 'dart:typed_data';

final pointData = Float32List.fromList([
  // x, y, z, value
  0.0, 0.0, 0.0, 0.0,
  1.0, 0.0, 0.5, 0.5,
  2.0, 0.0, 1.0, 1.0,
  3.0, 0.0, 0.5, 0.25,
]);

_controller.setPointBatches(
  points: pointData,
  batches: const [
    PointGlassOpenGLPointBatch(
      first: 0,
      count: 2,
      pointSize: 3.0,
      opacity: 1.0,
      valueMin: 0.0,
      valueMax: 1.0,
      colorMode: PointGlassOpenGLPointsColorMode.viridis,
    ),
    PointGlassOpenGLPointBatch(
      first: 2,
      count: 2,
      pointSize: 5.0,
      opacity: 0.8,
      valueMin: 0.0,
      valueMax: 1.0,
      colorMode: PointGlassOpenGLPointsColorMode.turbo2,
    ),
  ],
);
```

Each batch specifies a point range, not a `Float32List` offset. `first` is the starting point index and `count` is the number of points. Batch ranges must be sorted, non-overlapping, and within the submitted point buffer. Gaps between batches are not rendered; an empty batch list hides all points.

### Updating Batch Settings

Use `updatePointBatches()` to change the rendering configuration without retransmitting the point cloud data:

```dart
_controller.updatePointBatches(const [
  PointGlassOpenGLPointBatch(
    first: 0,
    count: 2,
    colorMode: PointGlassOpenGLPointsColorMode.viridis2,
    valueMin: 0.0,
    valueMax: 1.0,
  ),
  PointGlassOpenGLPointBatch(
    first: 2,
    count: 2,
    colorMode: PointGlassOpenGLPointsColorMode.coolwarm2,
    pointSize: 5.0,
    valueMin: 0.0,
    valueMax: 1.0,
  ),
]);
```

The batch settings are validated against the current point count. `updatePointBatches()` changes the rendering settings only; it does not re-upload point data. Calling `setPoints()` again switches back to single point cloud rendering.

## Display Parameters

Point cloud display properties can be updated in real time.

The `valueMin` and `valueMax` values are used by shader-based color mapping modes such as `viridis`.

```dart
_controller.setPointCloudDisplayParams(
  0.8, // alpha
  3.0, // point size
  -2.0, // minimum value for color mapping
  5.0, // maximum value for color mapping
  PointGlassOpenGLPointsColorMode.viridis,
);
```

For per-batch rendering, configure these properties on each `PointGlassOpenGLPointBatch` instead.

## Color Map Variants

Version 0.4.1 adds alternate visual variants for Viridis, Turbo, Rainbow, and Coolwarm. Each option is available through `PointGlassOpenGLPointsColorMode`:

| Color Map | Enum | Mode |
| --------- | ---- | ---- |
| Viridis | `viridis` | 0 |
| Viridis 2 | `viridis2` | 1 |
| Turbo | `turbo` | 2 |
| Turbo 2 | `turbo2` | 3 |
| Rainbow | `rainbow` | 4 |
| Rainbow 2 | `rainbow2` | 5 |
| Coolwarm | `coolwarm` | 6 |
| Coolwarm 2 | `coolwarm2` | 7 |
| Grayscale | `grayscale` | 8 |

Select a color map with `setPointCloudDisplayParams()` for a single point cloud or with the batch `colorMode` property for per-batch rendering.

**Compatibility note:** The enum names for the original modes remain available, but the numeric IDs for Turbo, Rainbow, Coolwarm, and Grayscale have changed. Applications that store raw numeric mode IDs must update their mappings.

## Background Color

`PointGlassOpenGLViewer` supports a configurable OpenGL background color through the `bgColor` option.

```dart
PointGlassOpenGLViewer(
  controller: _controller,
  bgColor: Colors.black,
  grid: PointGlassOpenGLGrid(),
  axis: PointGlassOpenGLAxis(),
);
```

The default background color is `Color(0xFF1A1A1A)`.

## X-axis Flip

The controller can flip the entire rendered scene along the X axis. This includes point clouds, grids, axes, and 3D-to-screen label projections.

```dart
final controller = PointGlassOpenGLController();

controller.setFlipX(true);  // Flip the X axis relative to the original viewer.
controller.setFlipX(false); // Restore the original viewer orientation (default).

final isFlipped = controller.flipX;
```

The flip can be configured before native renderer initialization or changed at runtime. It changes the renderer's MVP transformation only; source point coordinates are preserved and point cloud buffers do not need to be re-uploaded.

## Primitive Data Format

Lines and polygons use the following format:

```text
[X, Y, Z, R, G, B, A, Size/Thickness]
```

Example:

```dart
final lineData = Float32List.fromList([
  // x, y, z, r, g, b, a, thickness
  -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 2.0,
   1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 2.0,
]);

_controller.setLines(lineData);
```

```dart
final polygonData = Float32List.fromList([
  // x, y, z, r, g, b, a, size
  0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.5, 1.0,
  1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.5, 1.0,
  0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.5, 1.0,
]);

_controller.setPolygons(polygonData);
```

## Camera Controls

The controller provides camera APIs that can be connected to mouse, touch, keyboard, or custom input handlers.

```dart
_controller.changeCameraAngle(deltaX, deltaY);
_controller.panCamera(deltaX, deltaY);
_controller.rollCamera(deltaZ);
_controller.changeCameraZoom(scaleFactor);
```

### Interaction Examples

| Action | API                                 |
| ------ | ----------------------------------- |
| Orbit  | `changeCameraAngle(deltaX, deltaY)` |
| Pan    | `panCamera(deltaX, deltaY)`         |
| Roll   | `rollCamera(deltaZ)`                |
| Zoom   | `changeCameraZoom(scaleFactor)`     |

## Joystick Controls

`PointGlassOpenGLViewer` provides optional dual joystick controls.

```dart
PointGlassOpenGLViewer(
  controller: _controller,
  grid: PointGlassOpenGLGrid(),
  axis: PointGlassOpenGLAxis(),
  enableJoystick: true,
);
```

The left joystick controls XY movement, and the right joystick controls camera rotation.

Joystick controls are disabled by default.

## Mouse Coordinate Display

`PointGlassOpenGLViewer` can display the X/Y coordinate corresponding to the current mouse position on the ground plane.

The coordinate overlay is enabled by default.

```dart
PointGlassOpenGLViewer(
  controller: _controller,
  grid: PointGlassOpenGLGrid(),
  axis: PointGlassOpenGLAxis(),
  enableMouseCoordinate: true,
);
```

To hide the coordinate overlay:

```dart
PointGlassOpenGLViewer(
  controller: _controller,
  enableMouseCoordinate: false,
);
```

## Viewer Components

`point_glass_opengl` includes several built-in visualization components:

* `PointGlassOpenGLViewer` for rendering the 3D scene.
* `PointGlassOpenGLController` for updating data and controlling the viewer.
* `PointGlassOpenGLPointBatch` for configuring per-batch point cloud rendering.
* `PointGlassOpenGLGrid` for 3D grid rendering.
* `PointGlassOpenGLAxis` for X/Y/Z axis rendering.
* `PointGlassOpenGLLabel` for projecting 3D labels onto the 2D screen.
* `PointGlassOpenGLPoints` for point cloud model data.

## Architecture

This package uses a native rendering pipeline designed for real-time point cloud visualization.

```text
Flutter UI Layer
  - Declarative viewer models
  - User input handling
  - Controller API and batch display configuration

Platform Integration Layer
  - Windows/Linux Flutter texture integration
  - Android native surface integration
  - Dart-to-Rust FFI bindings

Rust Core Layer
  - OpenGL and OpenGL ES rendering
  - Shader and buffer management
  - Shared point cloud VBO with per-batch draw calls
  - 3D math and projection
```

## Native Library Compatibility

The Dart package and native Rust libraries must use compatible versions.

* Version 0.4.0 introduced the `set_flip_x` FFI function.
* Version 0.4.1 introduced `set_point_batches` and `update_point_batches`, along with support for color modes 0–8.

Update the Dart package and corresponding native Rust binaries together.

## Notes

* Windows, Linux, and Android are currently supported.
* macOS, iOS, and Web are not supported.
* Mouse coordinates are displayed relative to the ground plane.
* Dual joystick controls are disabled by default and can be enabled with `enableJoystick`.
* The OpenGL background color can be configured with `bgColor`.
* X-axis flipping is disabled by default and can be toggled with `PointGlassOpenGLController.setFlipX(bool)`.
* Point cloud data must be provided as `Float32List`.
* Point data uses 4 float values per point: `X`, `Y`, `Z`, and `Value`.
* Batch `first` and `count` values are measured in points, not float offsets.
* Batch ranges must be ordered, non-overlapping, and within the available point data.
* `updatePointBatches()` updates display settings without retransmitting point cloud data.
* Line and polygon data use 8 float values per vertex: `X`, `Y`, `Z`, `R`, `G`, `B`, `A`, and `Size/Thickness`.

## Examples

Check out the example project for a complete working implementation.

```bash
cd example
flutter pub get
flutter run
```

## Additional Information

* [GitHub Repository](https://github.com/immsong/point_glass_opengl)
* [Issue Tracker](https://github.com/immsong/point_glass_opengl/issues)
* [Documentation](https://github.com/immsong/point_glass_opengl#readme)
