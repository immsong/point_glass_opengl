import 'dart:ffi';
import 'dart:io';
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

class PointGlassController {
  static const MethodChannel _channel = MethodChannel('point_glass_opengl');

  Pointer<Void>? _rendererPtr;
  int? textureId;
  late DynamicLibrary _dylib;
  late CreateRendererDart _createRenderer;
  late SetPointsDart _setPoints;
  late UpdateCameraDart _updateCamera;

  // 카메라 상태값 보관
  double yaw = 0.0;
  double pitch = 0.0;
  double radius = 2.5;

  Future<void> initialize() async {
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

    // 포인터 주소를 C++로 넘기기 위해 함수 레퍼런스도 추출합니다.
    final renderFuncPointer = _dylib.lookup<NativeFunction<RenderFrameC>>(
      'render_frame',
    );

    _rendererPtr = _createRenderer();

    // --- 수정된 부분: C++에 Texture 생성을 요청하면서 Rust의 메모리 주소들을 함께 전달 ---
    textureId = await _channel.invokeMethod<int>('createTexture', {
      'rendererPtr': _rendererPtr!.address,
      'renderFuncPtr': renderFuncPointer.address,
    });
  }

  void updatePoints(Float32List points) {
    if (_rendererPtr == null) return;

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
      // 💡 핵심: FFI 직접 호출 대신, C++에 렌더링 신호 전송!
      _channel.invokeMethod('requestRender');
    }
  }

  // 마우스 드래그로 시점 회전
  void changeCameraAngle(double deltaX, double deltaY) {
    if (_rendererPtr == null) return;
    yaw += deltaX * 0.01;
    pitch -= deltaY * 0.01;
    _updateCamera(_rendererPtr!, yaw, pitch, radius);
    render(); // 카메라가 움직였으니 즉시 화면 갱신!
  }

  // 휠 스크롤로 줌인/줌아웃
  void changeCameraZoom(double deltaZoom) {
    if (_rendererPtr == null) return;
    radius += deltaZoom;
    _updateCamera(_rendererPtr!, yaw, pitch, radius);
    render();
  }
}

// 사용자에게 노출될 Flutter View 위젯
class PointGlassView extends StatefulWidget {
  final PointGlassController controller;

  const PointGlassView({Key? key, required this.controller}) : super(key: key);

  @override
  State<PointGlassView> createState() => _PointGlassViewState();
}

class _PointGlassViewState extends State<PointGlassView> {
  bool _isInitialized = false;

  @override
  void initState() {
    super.initState();
    widget.controller.initialize().then((_) {
      setState(() {
        _isInitialized = true;
      });
      // 테스트용: 초기화 완료 직후 한 프레임 렌더링
      widget.controller.render();
    });
  }

  @override
  Widget build(BuildContext context) {
    if (!_isInitialized || widget.controller.textureId == null) {
      return const Center(child: CircularProgressIndicator());
    }
    // 발급받은 textureId를 기반으로 Flutter 화면에 도화지를 띄움
    return Texture(textureId: widget.controller.textureId!);
  }
}
