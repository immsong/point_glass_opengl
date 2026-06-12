import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/gestures.dart';

import 'package:vector_math/vector_math.dart' as vm;

import 'package:point_glass_opengl/src/core/point_glass_opengl_controller.dart';
import 'package:point_glass_opengl/src/core/point_glass_opengl_raw_view.dart';
import 'package:point_glass_opengl/src/core/data_converter.dart';
import 'package:point_glass_opengl/src/models/point_glass_opengl_points.dart';
import 'package:point_glass_opengl/src/models/point_glass_opengl_grid.dart';
import 'package:point_glass_opengl/src/models/point_glass_opengl_label.dart';

/// 마우스/키보드 카메라 제어가 내장된 OpenGL 뷰어
class PointGlassOpenGLViewer extends StatefulWidget {
  final PointGlassOpenGLController? controller;

  final List<PointGlassOpenGLPoints>? pointsGroup;
  final PointGlassOpenGLGrid? grid;
  final List<PointGlassOpenGLLabel>? labels;

  const PointGlassOpenGLViewer({
    super.key,
    this.pointsGroup,
    this.grid,
    this.labels,
    this.controller,
  });

  @override
  State<PointGlassOpenGLViewer> createState() => _PointGlassOpenGLViewerState();
}

class _PointGlassOpenGLViewerState extends State<PointGlassOpenGLViewer> {
  late final PointGlassOpenGLController _controller;
  bool _isShiftPressed = false;
  bool _isCtrlPressed = false;

  List<PointGlassOpenGLLabel> _cachedLabels = [];

  @override
  void initState() {
    super.initState();
    _controller = widget.controller ?? PointGlassOpenGLController();

    HardwareKeyboard.instance.addHandler(_handleKeyEvent);
    _updateLabels();
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

    if (widget.grid != null) {
      final gridData = DataConverter.convertGrid(widget.grid!);
      _controller.setLines(gridData);
    } else {
      _controller.setLines(Float32List(0));
    }
  }

  void _updateLabels() {
    List<PointGlassOpenGLLabel> newLabels = [];

    // Grid 라벨 조립
    if (widget.grid != null && widget.grid!.enableLabel) {
      final int stepCount =
          (widget.grid!.gridSize / 2.0 / widget.grid!.gridStep).floor();
      for (int i = 0; i <= stepCount; i++) {
        double yPos = i * widget.grid!.gridStep;
        newLabels.add(
          PointGlassOpenGLLabel(
            position: vm.Vector3(0.0, yPos, 0.0),
            text: '${yPos.toStringAsFixed(0)}m',
            style: TextStyle(
              color: Colors.white70,
              fontSize: 12,
              fontWeight: FontWeight.bold,
              shadows: const [Shadow(color: Colors.black, blurRadius: 2)],
            ),
          ),
        );
      }
    }

    // 유저 커스텀 라벨 조립
    if (widget.labels != null) {
      newLabels.addAll(widget.labels!);
    }

    // 상태에 캐싱
    _cachedLabels = newLabels;
  }

  @override
  void didUpdateWidget(covariant PointGlassOpenGLViewer oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.pointsGroup != oldWidget.pointsGroup ||
        widget.grid != oldWidget.grid) {
      _updateData();
    }

    if (widget.grid != oldWidget.grid || widget.labels != oldWidget.labels) {
      _updateLabels();
    }
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
      child: Stack(
        children: [
          Positioned.fill(
            child: PointGlassOpenGLRawView(
              controller: _controller,
              onInitialized: _updateData,
            ),
          ),
          if (_cachedLabels.isNotEmpty)
            Positioned.fill(
              child: IgnorePointer(
                child: CustomPaint(
                  painter: _BatchLabelPainter(
                    controller: _controller,
                    labels: _cachedLabels,
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }
}

class _BatchLabelPainter extends CustomPainter {
  final PointGlassOpenGLController controller;
  final List<PointGlassOpenGLLabel> labels;

  _BatchLabelPainter({required this.controller, required this.labels})
    : super(repaint: controller);

  @override
  void paint(Canvas canvas, Size size) {
    if (labels.isEmpty) return;

    // 1. 3D 좌표만 쫙 뽑음
    final List<vm.Vector3> positions3D = labels.map((e) => e.position).toList();

    // 2. 일괄 변환 (Batch)
    final List<Offset?> offsetsNDC = controller.project3DToScreenBatch(
      positions3D,
    );

    // 3. 그리기
    for (int i = 0; i < labels.length; i++) {
      final ndc = offsetsNDC[i];
      if (ndc == null) continue;

      final screenX = (ndc.dx + 1.0) / 2.0 * size.width;
      final screenY = (ndc.dy + 1.0) / 2.0 * size.height;

      if (screenX < 0 ||
          screenX > size.width ||
          screenY < 0 ||
          screenY > size.height) {
        continue;
      }

      final label = labels[i];
      final textSpan = TextSpan(
        text: label.text,
        style:
            label.style ?? const TextStyle(color: Colors.white, fontSize: 12),
      );

      final textPainter = TextPainter(
        text: textSpan,
        textDirection: TextDirection.ltr,
      );
      textPainter.layout();

      textPainter.paint(
        canvas,
        Offset(
          screenX - (textPainter.width / 2),
          screenY - (textPainter.height / 2),
        ),
      );
    }
  }

  @override
  bool shouldRepaint(covariant _BatchLabelPainter oldDelegate) => true;
}
