import 'dart:ffi';
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

// Rust Core와 통신할 FFI 함수들 (간이 선언)
typedef CreateRendererC = Pointer<Void> Function();
typedef CreateRendererDart = Pointer<Void> Function();

typedef RenderFrameC = Void Function(Pointer<Void> renderer);
typedef RenderFrameDart = void Function(Pointer<Void> renderer);

class PointGlassController {
  static const MethodChannel _channel = MethodChannel('point_glass_opengl');

  Pointer<Void>? _rendererPtr;
  int? textureId;
  late DynamicLibrary _dylib;
  late CreateRendererDart _createRenderer;
  late RenderFrameDart _renderFrame;

  Future<void> initialize() async {
    // 1. Rust 라이브러리 로드 (테스트용 로컬 경로 유지)
    _dylib = DynamicLibrary.open(
      Platform.isWindows
          ? '../rust/target/debug/point_glass_opengl_core.dll'
          : '../rust/target/debug/libpoint_glass_opengl_core.so',
    );

    _createRenderer = _dylib
        .lookup<NativeFunction<CreateRendererC>>('create_renderer')
        .asFunction();
    _renderFrame = _dylib
        .lookup<NativeFunction<RenderFrameC>>('render_frame')
        .asFunction();

    // 2. Rust 측 렌더러 인스턴스 생성
    _rendererPtr = _createRenderer();

    // 3. 네이티브(Windows C++)에 텍스처 생성 요청 및 ID 수신
    // 이 ID를 통해 Flutter Texture 위젯과 연결합니다.
    textureId = await _channel.invokeMethod<int>('createTexture');
  }

  void render() {
    if (_rendererPtr != null) {
      // Rust 측에 한 프레임 그리도록 명령
      _renderFrame(_rendererPtr!);
    }
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
