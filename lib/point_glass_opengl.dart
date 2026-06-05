import 'dart:ffi';
import 'dart:io';
import 'dart:math';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:ffi/ffi.dart';

typedef CreateRendererC = Pointer<Void> Function();
typedef CreateRendererDart = Pointer<Void> Function();
typedef RenderFrameC = Void Function(Pointer<Void> renderer);
typedef RenderFrameDart = void Function(Pointer<Void> renderer);

// 💡 공통 데이터 전송 FFI
typedef SetDataC =
    Void Function(Pointer<Void> renderer, Pointer<Float> data, IntPtr length);
typedef SetDataDart =
    void Function(Pointer<Void> renderer, Pointer<Float> data, int length);

typedef UpdateCameraC =
    Void Function(
      Pointer<Void> renderer,
      Float yaw,
      Float pitch,
      Float roll,
      Float radius,
    );
typedef UpdateCameraDart =
    void Function(
      Pointer<Void> renderer,
      double yaw,
      double pitch,
      double roll,
      double radius,
    );
typedef ResizeRendererC =
    Void Function(Pointer<Void> renderer, Uint32 width, Uint32 height);
typedef ResizeRendererDart =
    void Function(Pointer<Void> renderer, int width, int height);
typedef PanCameraC = Void Function(Pointer<Void> renderer, Float dx, Float dy);
typedef PanCameraDart =
    void Function(Pointer<Void> renderer, double dx, double dy);

class PointGlassController {
  static const MethodChannel _channel = MethodChannel('point_glass_opengl');

  Pointer<Void>? _rendererPtr;
  int? textureId;
  late DynamicLibrary _dylib;
  late CreateRendererDart _createRenderer;

  // 💡 3가지 바인딩 함수
  late SetDataDart _setPoints;
  late SetDataDart _setLines;
  late SetDataDart _setPolygons;

  late UpdateCameraDart _updateCamera;
  late ResizeRendererDart _resizeRenderer;
  late PanCameraDart _panCamera;

  double yaw = pi;
  double pitch = pi;
  double roll = 0.0;
  double radius = 8.0;

  Future<void> initialize({int width = 400, int height = 400}) async {
    _dylib = DynamicLibrary.open(
      Platform.isWindows
          ? '../rust/target/debug/point_glass_opengl_core.dll'
          : '../rust/target/debug/libpoint_glass_opengl_core.so',
    );

    _createRenderer = _dylib
        .lookup<NativeFunction<CreateRendererC>>('create_renderer')
        .asFunction();
    _setPoints = _dylib
        .lookup<NativeFunction<SetDataC>>('set_points')
        .asFunction();
    _setLines = _dylib
        .lookup<NativeFunction<SetDataC>>('set_lines')
        .asFunction();
    _setPolygons = _dylib
        .lookup<NativeFunction<SetDataC>>('set_polygons')
        .asFunction();
    _updateCamera = _dylib
        .lookup<NativeFunction<UpdateCameraC>>('update_camera')
        .asFunction();
    _resizeRenderer = _dylib
        .lookup<NativeFunction<ResizeRendererC>>('resize_renderer')
        .asFunction();
    _panCamera = _dylib
        .lookup<NativeFunction<PanCameraC>>('pan_camera')
        .asFunction();

    final renderFuncPointer = _dylib.lookup<NativeFunction<RenderFrameC>>(
      'render_frame',
    );

    _rendererPtr = _createRenderer();
    _updateCamera(_rendererPtr!, yaw, pitch, roll, radius);
    _resizeRenderer(_rendererPtr!, width, height);

    textureId = await _channel.invokeMethod<int>('createTexture', {
      'rendererPtr': _rendererPtr!.address,
      'renderFuncPtr': renderFuncPointer.address,
      'width': width,
      'height': height,
    });
  }

  Future<void> resize(int width, int height) async {
    if (textureId == null || _rendererPtr == null) return;
    _resizeRenderer(_rendererPtr!, width, height);
    await _channel.invokeMethod('resizeTexture', {
      'width': width,
      'height': height,
    });
    render();
  }

  void render() {
    if (textureId != null) {
      _channel.invokeMethod('requestRender');
    }
  }

  // 내부 통신 헬퍼
  void _sendDataToRust(SetDataDart ffiFunc, Float32List data) {
    if (_rendererPtr == null || data.isEmpty) return;
    final pointer = calloc<Float>(data.length);
    pointer.asTypedList(data.length).setAll(0, data);
    ffiFunc(_rendererPtr!, pointer, data.length);
    calloc.free(pointer);
  }

  // 💡 사용자가 호출할 완벽한 범용 API 3개!
  void setPoints(Float32List points) {
    _sendDataToRust(_setPoints, points);
    render();
  }

  void setLines(Float32List lines) {
    _sendDataToRust(_setLines, lines);
    render();
  }

  void setPolygons(Float32List polygons) {
    _sendDataToRust(_setPolygons, polygons);
    render();
  }

  void changeCameraAngle(double deltaX, double deltaY) {
    if (_rendererPtr == null) return;
    yaw = (yaw + (deltaX * 0.0015)) % (pi * 2);
    pitch = (pitch - deltaY * 0.003).clamp(pi, pi * 2);
    _updateCamera(_rendererPtr!, yaw, pitch, roll, radius);
    render();
  }

  void rollCamera(double deltaZ) {
    if (_rendererPtr == null) return;
    roll = (roll + (deltaZ * 0.0015)) % (pi * 2);
    _updateCamera(_rendererPtr!, yaw, pitch, roll, radius);
    render();
  }

  void panCamera(double screenDx, double screenDy) {
    if (_rendererPtr == null) return;
    _panCamera(_rendererPtr!, screenDx.toDouble(), screenDy.toDouble());
    render();
  }

  void changeCameraZoom(double scaleFactor) {
    if (_rendererPtr == null) return;
    radius = (radius * scaleFactor).clamp(0.1, double.infinity);
    _updateCamera(_rendererPtr!, yaw, pitch, roll, radius);
    render();
  }
}

class PointGlassView extends StatefulWidget {
  final PointGlassController controller;
  final VoidCallback? onInitialized;

  const PointGlassView({
    super.key,
    required this.controller,
    this.onInitialized,
  });

  @override
  State<PointGlassView> createState() => _PointGlassViewState();
}

class _PointGlassViewState extends State<PointGlassView> {
  bool _isInitialized = false;
  bool _initStarted = false;
  int _lastWidth = 0;
  int _lastHeight = 0;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final ratio = MediaQuery.of(context).devicePixelRatio;
        final w = (constraints.maxWidth * ratio).round();
        final h = (constraints.maxHeight * ratio).round();

        if (w <= 0 || h <= 0) {
          return const Center(child: CircularProgressIndicator());
        }

        if (!_initStarted) {
          _initStarted = true;
          widget.controller.initialize(width: w, height: h).then((_) {
            if (mounted) setState(() => _isInitialized = true);
            widget.controller.render();
            widget.onInitialized?.call();
          });
        }

        if (!_isInitialized || widget.controller.textureId == null) {
          return const Center(child: CircularProgressIndicator());
        }

        if (_lastWidth != 0 && (_lastWidth != w || _lastHeight != h)) {
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (mounted) widget.controller.resize(w, h);
          });
        }
        _lastWidth = w;
        _lastHeight = h;

        return Texture(textureId: widget.controller.textureId!);
      },
    );
  }
}
