import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/gestures.dart';

import 'package:point_glass_opengl/src/core/point_glass_opengl_controller.dart';
import 'package:point_glass_opengl/src/core/point_glass_opengl_raw_view.dart';
import 'package:point_glass_opengl/src/core/data_converter.dart';
import 'package:point_glass_opengl/src/models/point_glass_opengl_points.dart';

/// 마우스/키보드 카메라 제어가 내장된 OpenGL 뷰어
class PointGlassOpenGLViewer extends StatefulWidget {
  final List<PointGlassOpenGLPoints>? pointsGroup;

  const PointGlassOpenGLViewer({super.key, this.pointsGroup});

  @override
  State<PointGlassOpenGLViewer> createState() => _PointGlassOpenGLViewerState();
}

class _PointGlassOpenGLViewerState extends State<PointGlassOpenGLViewer> {
  final PointGlassOpenGLController _controller = PointGlassOpenGLController();
  bool _isShiftPressed = false;
  bool _isCtrlPressed = false;

  @override
  void initState() {
    super.initState();
    HardwareKeyboard.instance.addHandler(_handleKeyEvent);
  }

  @override
  void dispose() {
    HardwareKeyboard.instance.removeHandler(_handleKeyEvent);
    super.dispose();
  }

  bool _handleKeyEvent(KeyEvent event) {
    if (event.logicalKey == LogicalKeyboardKey.shiftLeft ||
        event.logicalKey == LogicalKeyboardKey.shiftRight) {
      if (mounted) {
        setState(
          () => _isShiftPressed =
              event is KeyDownEvent || event is KeyRepeatEvent,
        );
      }

      return true;
    } else if (event.logicalKey == LogicalKeyboardKey.controlLeft ||
        event.logicalKey == LogicalKeyboardKey.controlRight) {
      if (mounted) {
        setState(
          () =>
              _isCtrlPressed = event is KeyDownEvent || event is KeyRepeatEvent,
        );
      }

      return true;
    }
    return false;
  }

  void _updateData() {
    if (widget.pointsGroup != null) {
      final floatData = DataConverter.convertPointsGroup(widget.pointsGroup!);
      _controller.setPoints(floatData);
    }
  }

  @override
  void didUpdateWidget(covariant PointGlassOpenGLViewer oldWidget) {
    super.didUpdateWidget(oldWidget);
    _updateData();
  }

  @override
  Widget build(BuildContext context) {
    return Listener(
      onPointerSignal: (event) {
        if (event is PointerScrollEvent) {
          final scaleFactor = event.scrollDelta.dy > 0 ? 1.1 : 0.9;
          _controller.changeCameraZoom(scaleFactor);
        }
      },
      onPointerMove: (event) {
        if (_isShiftPressed) {
          _controller.panCamera(-event.delta.dx, event.delta.dy);
        } else if (_isCtrlPressed) {
          _controller.rollCamera(-event.delta.dx);
        } else if (event.buttons == kPrimaryMouseButton) {
          _controller.changeCameraAngle(-event.delta.dx, event.delta.dy);
        }
      },
      child: PointGlassOpenGLRawView(
        controller: _controller,
        onInitialized: _updateData,
      ),
    );
  }
}
