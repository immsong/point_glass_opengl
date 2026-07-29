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
import 'package:point_glass_opengl/src/models/point_glass_opengl_axis.dart';

/// 마우스/키보드 카메라 제어가 내장된 OpenGL 뷰어
class PointGlassOpenGLViewer extends StatefulWidget {
  final PointGlassOpenGLController? controller;

  final List<PointGlassOpenGLPoints>? pointsGroup;
  final PointGlassOpenGLGrid? grid;
  final List<PointGlassOpenGLLabel>? labels;
  final PointGlassOpenGLAxis? axis;

  /// 마우스가 가리키는 Z 평면의 3D 좌표 표시 여부
  final bool enableMouseCoordinate;

  const PointGlassOpenGLViewer({
    super.key,
    this.pointsGroup,
    this.grid,
    this.labels,
    this.axis,
    this.controller,
    this.enableMouseCoordinate = true,
  });

  @override
  State<PointGlassOpenGLViewer> createState() => _PointGlassOpenGLViewerState();
}

class _PointGlassOpenGLViewerState extends State<PointGlassOpenGLViewer> {
  late final PointGlassOpenGLController _controller;

  bool _isShiftPressed = false;
  bool _isCtrlPressed = false;

  /// 뷰어 내부 기준 마우스 위치
  Offset? _mousePosition;
  vm.Vector3? _mouseWorldPosition;

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
        setState(() {
          _isShiftPressed = event is KeyDownEvent || event is KeyRepeatEvent;
        });
      }

      return true;
    }

    if (event.logicalKey == LogicalKeyboardKey.controlLeft ||
        event.logicalKey == LogicalKeyboardKey.controlRight) {
      if (mounted) {
        setState(() {
          _isCtrlPressed = event is KeyDownEvent || event is KeyRepeatEvent;
        });
      }

      return true;
    }

    return false;
  }

  void _updateMousePosition(
    Offset position,
    Size viewerSize,
  ) {
    final worldPosition = _controller.screenToWorldOnPlane(
      screenPosition: position,
      viewportSize: viewerSize,
      planeZ: 0.0,
    );

    if (!mounted) {
      return;
    }

    setState(() {
      _mousePosition = position;
      _mouseWorldPosition = worldPosition;
    });
  }

  void _clearMousePosition() {
    if (!mounted) {
      return;
    }

    setState(() {
      _mousePosition = null;
      _mouseWorldPosition = null;
    });
  }

  void _updateData() {
    if (widget.pointsGroup != null) {
      final floatData = DataConverter.convertPointsGroup(
        widget.pointsGroup!,
      );

      _controller.setPoints(floatData);
    }

    if (widget.grid != null) {
      final gridData = DataConverter.convertGrid(widget.grid!);

      _controller.setLines(gridData);
    } else {
      _controller.setLines(Float32List(0));
    }

    if (widget.axis != null) {
      final axisData = DataConverter.convertAxisToPolygons(
        widget.axis!,
      );

      _controller.setPolygons(axisData);
    } else {
      _controller.setPolygons(Float32List(0));
    }

    _controller.render();
  }

  void _updateLabels() {
    final List<PointGlassOpenGLLabel> newLabels = [];

    // Grid 라벨 조립
    if (widget.grid != null && widget.grid!.enableLabel) {
      final int stepCount =
          (widget.grid!.gridSize / 2.0 / widget.grid!.gridStep).floor();

      for (int i = 0; i <= stepCount; i++) {
        final double yPos = i * widget.grid!.gridStep;

        newLabels.add(
          PointGlassOpenGLLabel(
            position: vm.Vector3(
              0.0,
              yPos,
              0.0,
            ),
            text: '${yPos.toStringAsFixed(0)}m',
            style: const TextStyle(
              color: Colors.white70,
              fontSize: 12,
              fontWeight: FontWeight.bold,
              shadows: [
                Shadow(
                  color: Colors.black,
                  blurRadius: 2,
                ),
              ],
            ),
          ),
        );
      }
    }

    // Axis 라벨 조립
    if (widget.axis != null && widget.axis!.labelEnable) {
      // X축 라벨
      newLabels.add(widget.axis!.labelX);
      // Y축 라벨
      newLabels.add(widget.axis!.labelY);
      // Z축 라벨
      newLabels.add(widget.axis!.labelZ);
    }

    // 사용자 정의 라벨 조립
    if (widget.labels != null) {
      newLabels.addAll(widget.labels!);
    }

    _cachedLabels = newLabels;
  }

  @override
  void didUpdateWidget(
    covariant PointGlassOpenGLViewer oldWidget,
  ) {
    super.didUpdateWidget(oldWidget);

    if (oldWidget.enableMouseCoordinate && !widget.enableMouseCoordinate) {
      _mousePosition = null;
      _mouseWorldPosition = null;
    }

    if (widget.pointsGroup != oldWidget.pointsGroup ||
        widget.grid != oldWidget.grid ||
        widget.axis != oldWidget.axis) {
      _updateData();
    }

    if (widget.grid != oldWidget.grid ||
        widget.labels != oldWidget.labels ||
        widget.axis != oldWidget.axis) {
      _updateLabels();
    }
  }

  Widget _buildMousePositionOverlay(
    Size viewerSize,
  ) {
    final screenPosition = _mousePosition!;
    final worldPosition = _mouseWorldPosition;

    const double margin = 12.0;
    const double overlayWidth = 100.0;
    const double overlayHeight = 50.0;

    double left = screenPosition.dx + margin;
    double top = screenPosition.dy + margin;

    if (left + overlayWidth > viewerSize.width) {
      left = screenPosition.dx - overlayWidth - margin;
    }

    if (top + overlayHeight > viewerSize.height) {
      top = screenPosition.dy - overlayHeight - margin;
    }

    left = left
        .clamp(
          0.0,
          (viewerSize.width - overlayWidth).clamp(
            0.0,
            double.infinity,
          ),
        )
        .toDouble();

    top = top
        .clamp(
          0.0,
          (viewerSize.height - overlayHeight).clamp(
            0.0,
            double.infinity,
          ),
        )
        .toDouble();

    final coordinateText = worldPosition == null
        ? ''
        : 'X: ${(-worldPosition.x).toStringAsFixed(2)} m\n'
            'Y: ${worldPosition.y.toStringAsFixed(2)} m';

    return Positioned(
      left: left,
      top: top,
      width: overlayWidth,
      child: IgnorePointer(
        child: Container(
          padding: const EdgeInsets.symmetric(
            horizontal: 8,
            vertical: 6,
          ),
          decoration: BoxDecoration(
            color: Colors.black.withAlpha(100),
            borderRadius: BorderRadius.all(Radius.circular(4)),
            border: Border.all(
              color: Colors.white.withAlpha(100),
            ),
          ),
          child: Text(
            coordinateText,
            style: const TextStyle(
              color: Colors.white,
              fontSize: 11,
            ),
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (
        BuildContext context,
        BoxConstraints constraints,
      ) {
        final viewerSize = Size(
          constraints.maxWidth,
          constraints.maxHeight,
        );

        return MouseRegion(
          onHover: (PointerHoverEvent event) {
            _updateMousePosition(
              event.localPosition,
              viewerSize,
            );
          },
          onExit: (_) {
            _clearMousePosition();
          },
          child: Listener(
            behavior: HitTestBehavior.opaque,
            onPointerSignal: (
              PointerSignalEvent event,
            ) {
              if (event is PointerScrollEvent) {
                final scaleFactor = event.scrollDelta.dy > 0 ? 1.1 : 0.9;

                _controller.changeCameraZoom(
                  scaleFactor,
                );
              }
            },
            onPointerMove: (
              PointerMoveEvent event,
            ) {
              // 드래그 중에는 MouseRegion.onHover가
              // 호출되지 않을 수 있으므로 여기서도 갱신
              _updateMousePosition(
                event.localPosition,
                viewerSize,
              );

              if (_isShiftPressed) {
                _controller.panCamera(
                  -event.delta.dx,
                  event.delta.dy,
                );
              } else if (_isCtrlPressed) {
                _controller.rollCamera(
                  -event.delta.dx,
                );
              } else if (event.buttons == kPrimaryMouseButton) {
                _controller.changeCameraAngle(
                  -event.delta.dx,
                  event.delta.dy,
                );
              }
            },
            child: Stack(
              clipBehavior: Clip.hardEdge,
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
                if (widget.enableMouseCoordinate && _mousePosition != null)
                  _buildMousePositionOverlay(viewerSize),
              ],
            ),
          ),
        );
      },
    );
  }
}

class _BatchLabelPainter extends CustomPainter {
  final PointGlassOpenGLController controller;
  final List<PointGlassOpenGLLabel> labels;

  _BatchLabelPainter({
    required this.controller,
    required this.labels,
  }) : super(repaint: controller);

  @override
  void paint(Canvas canvas, Size size) {
    if (labels.isEmpty) {
      return;
    }

    // 3D 라벨 위치 추출
    final List<vm.Vector3> positions3D =
        labels.map((label) => label.position).toList();

    // 3D 좌표를 NDC 좌표로 일괄 변환
    final List<Offset?> offsetsNDC = controller.project3DToScreenBatch(
      positions3D,
    );

    if (offsetsNDC.isEmpty || offsetsNDC.length != labels.length) {
      return;
    }

    for (int i = 0; i < labels.length; i++) {
      final ndc = offsetsNDC[i];

      if (ndc == null) {
        continue;
      }

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
        style: label.style ??
            const TextStyle(
              color: Colors.white,
              fontSize: 12,
            ),
      );

      final textPainter = TextPainter(
        text: textSpan,
        textDirection: TextDirection.ltr,
      );

      textPainter.layout();

      textPainter.paint(
        canvas,
        Offset(
          screenX - textPainter.width / 2,
          screenY - textPainter.height / 2,
        ),
      );
    }
  }

  @override
  bool shouldRepaint(
    covariant _BatchLabelPainter oldDelegate,
  ) {
    return oldDelegate.labels != labels || oldDelegate.controller != controller;
  }
}
