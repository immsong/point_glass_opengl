# point_glass_opengl

A Flutter package for high-performance 3D point cloud visualization using native Rust and OpenGL rendering.

`point_glass_opengl` provides a native offscreen rendering pipeline for visualizing point cloud data in Flutter desktop applications. It supports OpenGL-based rendering, interactive 3D camera controls, grid and axis overlays, label projection, and real-time display parameter updates.

## Features

* **Native 3D Rendering**: Rust-based OpenGL rendering core for high-performance 3D visualization.
* **Point Cloud Rendering**: FFI-based point cloud rendering pipeline with VBO support.
* **Offscreen Rendering**: FBO- and `glReadPixels`-based rendering for Flutter texture integration.
* **3D Camera Controls**: Built-in orbit, pan, roll, and zoom controls.
* **Visualization Overlays**: Grid, axis, and 3D-to-2D label projection support.
* **Depth-based Color Mapping**: Real-time point cloud color mapping using a value range.
* **Display Controls**: Runtime control of point size, alpha, value range, and color mode.
* **Declarative Flutter API**: Flutter-friendly viewer models and external controller support.

## Platform Support

| Platform | Status            |
| -------- | ----------------- |
| Windows  | Supported         |
| Linux    | Supported         |
| macOS    | Not supported yet |
| Android  | Not supported yet |
| iOS      | Not supported yet |
| Web      | Not supported     |

## Installation

Add this to your package's `pubspec.yaml` file:

```yaml
dependencies:
  point_glass_opengl: ^0.1.0
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

## Viewer Components

`point_glass_opengl` includes several built-in visualization components:

* `PointGlassOpenGLViewer` for rendering the 3D scene.
* `PointGlassOpenGLController` for updating data and controlling the viewer.
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
  - Controller API

C++ Plugin Layer
  - Flutter texture integration
  - Platform-specific native bridge
  - Texture lifecycle management

Rust Core Layer
  - OpenGL context control
  - Shader and buffer management
  - Point cloud VBO rendering
  - 3D math and projection
```

## Notes

* This package currently focuses on Windows and Linux desktop environments.
* Web and mobile platforms are not supported in the current release.
* Point cloud data must be provided as `Float32List`.
* Point data uses 4 float values per point: `X`, `Y`, `Z`, and `Value`.
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
