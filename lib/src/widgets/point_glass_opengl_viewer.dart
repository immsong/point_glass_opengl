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
import 'package:point_glass_opengl/src/widgets/point_glass_opengl_mouse_coordinate_overlay.dart';
import 'package:point_glass_opengl/src/widgets/point_glass_opengl_label_painter.dart';
import 'package:point_glass_opengl/src/widgets/point_glass_opengl_dual_joystick.dart';

/// 마우스/키보드 카메라 제어가 내장된 OpenGL 뷰어
class PointGlassOpenGLViewer extends StatefulWidget {
  final PointGlassOpenGLController? controller;
  final Color bgColor;

  final List<PointGlassOpenGLPoints>? pointsGroup;
  final PointGlassOpenGLGrid? grid;
  final List<PointGlassOpenGLLabel>? labels;
  final PointGlassOpenGLAxis? axis;

  /// 마우스가 가리키는 Z 평면의 3D 좌표 표시 여부
  final bool enableMouseCoordinate;

  /// 조이스틱 모드 사용 여부
  final bool enableJoystick;

  const PointGlassOpenGLViewer({
    super.key,
    this.pointsGroup,
    this.grid,
    this.labels,
    this.axis,
    this.controller,
    this.bgColor = const Color(0xFF1A1A1A),
    this.enableMouseCoordinate = true,
    this.enableJoystick = true,
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

  // pinch gesture를 위한 이전 스케일 값
  double _previousTouchScale = 1.0;
  bool _isPinching = false;

  @override
  void initState() {
    super.initState();

    _controller = widget.controller ?? PointGlassOpenGLController();
    _controller.setBgColor(widget.bgColor);

    HardwareKeyboard.instance.addHandler(_handleKeyEvent);

    _updateLabels();
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

    if (widget.bgColor != oldWidget.bgColor) {
      _controller.setBgColor(widget.bgColor);
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

  Widget _buildViewerInteractionLayer(Size viewerSize, Widget viewer) {
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onScaleStart: (ScaleStartDetails details) {
        _previousTouchScale = 1.0;
      },
      onScaleUpdate: (ScaleUpdateDetails details) {
        // 두 손가락일 때만 pinch zoom 처리
        if (details.pointerCount != 2) {
          return;
        }

        _isPinching = true;

        // details.scale:
        // 손가락을 벌리면 1보다 커짐
        // 손가락을 모으면 1보다 작아짐
        //
        // 현재 changeCameraZoom은 1보다 작으면 확대,
        // 1보다 크면 축소하는 구조이므로 역비율 사용
        final scaleFactor = _previousTouchScale / details.scale;

        _previousTouchScale = details.scale;

        _controller.changeCameraZoom(
          scaleFactor.clamp(0.8, 1.2),
        );
      },
      onScaleEnd: (_) {
        _previousTouchScale = 1.0;
        _isPinching = false;
      },
      child: MouseRegion(
        onHover: (PointerHoverEvent event) {
          if (_isPinching) {
            return;
          }

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
            if (_isPinching) {
              return;
            }

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
            if (_isPinching) {
              return;
            }

            // 드래그 중에는 MouseRegion.onHover가
            // 호출되지 않을 수 있으므로 여기서도 갱신
            _updateMousePosition(
              event.localPosition,
              viewerSize,
            );

            if (widget.enableJoystick) {
              return;
            }

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
          child: viewer,
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

        return Stack(
          clipBehavior: Clip.hardEdge,
          children: [
            Positioned.fill(
              child: _buildViewerInteractionLayer(
                viewerSize,
                PointGlassOpenGLRawView(
                  controller: _controller,
                  onInitialized: _updateData,
                ),
              ),
            ),
            if (_cachedLabels.isNotEmpty)
              Positioned.fill(
                child: IgnorePointer(
                  child: CustomPaint(
                    painter: PointGlassOpenGLLabelPainter(
                      controller: _controller,
                      labels: _cachedLabels,
                    ),
                  ),
                ),
              ),
            if (widget.enableMouseCoordinate && _mousePosition != null)
              PointGlassOpenGLMouseCoordinateOverlay(
                screenPosition: _mousePosition!,
                worldPosition: _mouseWorldPosition,
                viewerSize: viewerSize,
              ),
            if (widget.enableJoystick)
              Positioned.fill(
                child: PointGlassOpenGLDualJoystick(
                  viewerSize: viewerSize,
                  updateStepLeft: 4.0,
                  onUpdateLeft: (Offset offset) {
                    _controller.panCamera(
                      offset.dx,
                      offset.dy,
                    );
                  },
                  updateStepRight: 4.0,
                  onUpdateRight: (Offset offset) {
                    _controller.changeCameraAngle(
                      -offset.dx,
                      -offset.dy,
                    );
                  },
                ),
              ),
          ],
        );
      },
    );
  }
}
