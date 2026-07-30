import 'package:flutter/material.dart';
import 'point_glass_opengl_controller.dart';

/// 제스처 처리 없이 텍스처 출력과 리사이즈만 담당하는 순수 하위 뷰
class PointGlassOpenGLRawView extends StatefulWidget {
  final PointGlassOpenGLController controller;
  final VoidCallback? onInitialized;

  const PointGlassOpenGLRawView({
    super.key,
    required this.controller,
    this.onInitialized,
  });

  @override
  State<PointGlassOpenGLRawView> createState() =>
      _PointGlassOpenGLRawViewState();
}

class _PointGlassOpenGLRawViewState extends State<PointGlassOpenGLRawView> {
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
            if (!mounted) {
              return;
            }

            // initialize에 사용한 크기 기록
            _lastWidth = w;
            _lastHeight = h;

            // 초기화 완료 후 viewport/projection 동기화
            widget.controller.resize(w, h);

            setState(() {
              _isInitialized = true;
            });

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
