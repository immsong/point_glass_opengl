import 'dart:ffi';
import 'dart:io';
import 'dart:math';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:ffi/ffi.dart';

// Rust Core와 통신할 FFI 함수들 (간이 선언)
typedef CreateRendererC = Pointer<Void> Function();
typedef CreateRendererDart = Pointer<Void> Function();

typedef RenderFrameC = Void Function(Pointer<Void> renderer);
typedef RenderFrameDart = void Function(Pointer<Void> renderer);

typedef SetPointsC =
    Void Function(Pointer<Void> renderer, Pointer<Float> data, IntPtr length);
typedef SetPointsDart =
    void Function(Pointer<Void> renderer, Pointer<Float> data, int length);

typedef UpdateCameraC =
    Void Function(Pointer<Void> renderer, Float yaw, Float pitch, Float radius);
typedef UpdateCameraDart =
    void Function(
      Pointer<Void> renderer,
      double yaw,
      double pitch,
      double radius,
    );

typedef ResizeRendererC =
    Void Function(Pointer<Void> renderer, Uint32 width, Uint32 height);
typedef ResizeRendererDart =
    void Function(Pointer<Void> renderer, int width, int height);

typedef PanCameraC = Void Function(Pointer<Void> renderer, Float dx, Float dy);
typedef PanCameraDart =
    void Function(Pointer<Void> renderer, double dx, double dy);

typedef RotateZC = Void Function(Pointer<Void> renderer, Float delta);
typedef RotateZDart = void Function(Pointer<Void> renderer, double delta);

class PointGlassController {
  static const MethodChannel _channel = MethodChannel('point_glass_opengl');

  Pointer<Void>? _rendererPtr;
  int? textureId;
  late DynamicLibrary _dylib;
  late CreateRendererDart _createRenderer;
  late SetPointsDart _setPoints;
  late UpdateCameraDart _updateCamera;
  late ResizeRendererDart _resizeRenderer;
  late PanCameraDart _panCamera;

  // 카메라 상태값 보관
  double yaw = 0.0;
  double pitch = -pi / 2; // -90° 시작
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
        .lookup<NativeFunction<SetPointsC>>('set_points')
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

    // 포인터 주소를 C++로 넘기기 위해 함수 레퍼런스도 추출합니다.
    final renderFuncPointer = _dylib.lookup<NativeFunction<RenderFrameC>>(
      'render_frame',
    );

    _rendererPtr = _createRenderer();

    // Dart의 초기 카메라 값을 Rust에 즉시 적용
    _updateCamera(_rendererPtr!, yaw, pitch, radius);

    // Rust Renderer에 초기 크기 전달
    _resizeRenderer(_rendererPtr!, width, height);

    // C++에 Texture 생성을 요청하면서 Rust 메모리 주소와 초기 해상도를 함께 전달
    textureId = await _channel.invokeMethod<int>('createTexture', {
      'rendererPtr': _rendererPtr!.address,
      'renderFuncPtr': renderFuncPointer.address,
      'width': width,
      'height': height,
    });
  }

  Future<void> resize(int width, int height) async {
    if (textureId == null || _rendererPtr == null) {
      return;
    }

    // Rust에 직접 크기 전달 (FFI, MethodChannel 거치지 않음)
    _resizeRenderer(_rendererPtr!, width, height);
    await _channel.invokeMethod('resizeTexture', {
      'width': width,
      'height': height,
    });
    render();
  }

  void updatePoints(Float32List points) {
    if (_rendererPtr == null) {
      return;
    }

    // Float32List를 C/Rust가 이해할 수 있는 메모리 포인터로 복사
    final pointer = calloc<Float>(points.length);
    final nativeList = pointer.asTypedList(points.length);
    nativeList.setAll(0, points);

    // Rust로 쏘기!
    _setPoints(_rendererPtr!, pointer, points.length);

    // 메모리 누수를 막기 위해 임시 포인터는 해제
    calloc.free(pointer);

    // 데이터를 업데이트했으니 화면을 다시 그리도록 요청
    render();
  }

  void render() {
    if (textureId != null) {
      // FFI 직접 호출 대신, C++에 렌더링 신호 전송
      _channel.invokeMethod('requestRender');
    }
  }

  // 마우스 드래그로 시점 회전
  void changeCameraAngle(double deltaX, double deltaY) {
    if (_rendererPtr == null) {
      return;
    }

    // 좌우 드래그
    yaw += deltaX * 0.0015;

    // 상하 드래그
    pitch = (pitch - deltaY * 0.003).clamp(-pi / 2, pi / 2);

    _updateCamera(_rendererPtr!, yaw, pitch, radius);
    render();
  }

  // Shift+드래그: 카메라 pan
  void panCamera(double screenDx, double screenDy) {
    if (_rendererPtr == null) {
      return;
    }

    _panCamera(_rendererPtr!, screenDx.toDouble(), screenDy.toDouble());
    render();
  }

  // 휠 스크롤로 줌인/줌아웃 (배율 기반: 0.9 = 축소, 1.1 = 확대)
  void changeCameraZoom(double scaleFactor) {
    if (_rendererPtr == null) return;
    radius = (radius * scaleFactor).clamp(0.1, double.infinity);
    _updateCamera(_rendererPtr!, yaw, pitch, radius);
    render();
  }
}

// 사용자에게 노출될 Flutter View 위젯
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
