import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';

class PointGlassOpenGLJoystick extends StatefulWidget {
  // 입력이 활성화된 동안 20ms마다 호출
  //
  // 각 축에는 입력 방향과 thumb 이동 거리에 따른 값이 전달됨
  //
  // 최대 이동 위치에서:
  // 왼쪽: -updateStep
  // 오른쪽: +updateStep
  // 아래쪽: -updateStep
  // 위쪽: +updateStep
  //
  // 중앙에 가까울수록 updateStep보다 작은 값이 전달됨
  final ValueChanged<Offset>? onUpdate;

  // 조이스틱 입력이 종료될 때 호출
  final VoidCallback? onReleased;

  // 한 번의 update마다 전달할 최대값
  final double updateStep;

  // 입력으로 인식하기 시작하는 최소 이동 비율
  //
  // 범위는 0.0 ~ 1.0
  final double deadZone;

  const PointGlassOpenGLJoystick({
    super.key,
    this.onUpdate,
    this.onReleased,
    this.updateStep = 1.0,
    this.deadZone = 0.1,
  })  : assert(updateStep > 0.0),
        assert(deadZone >= 0.0 && deadZone <= 1.0);

  @override
  State<PointGlassOpenGLJoystick> createState() =>
      _PointGlassOpenGLJoystickState();
}

class _PointGlassOpenGLJoystickState extends State<PointGlassOpenGLJoystick> {
  // 입력값 반복 전달 주기
  static const Duration _updateInterval = Duration(
    milliseconds: 20,
  );

  // 화면에 표시되는 thumb 위치
  Offset _thumbOffset = Offset.zero;

  // 각 축의 현재 입력 방향과 속도 배율
  Offset _inputDirection = Offset.zero;

  // 현재 조이스틱을 조작하고 있는 포인터
  int? _activePointer;

  // 최초로 터치한 위치
  Offset? _dragStartPosition;

  // 주기적인 입력 전달 여부
  bool _isInputActive = false;

  // 위젯이 존재하는 동안 하나만 유지되는 타이머
  late final Timer _updateTimer;

  @override
  void initState() {
    super.initState();

    _updateTimer = Timer.periodic(
      _updateInterval,
      (_) {
        _handlePeriodicUpdate();
      },
    );
  }

  @override
  void dispose() {
    _updateTimer.cancel();

    super.dispose();
  }

  // 현재 포인터를 조이스틱 입력 포인터로 등록
  //
  // 터치한 위치로 thumb를 이동시키지 않고
  // 이후 드래그 거리를 계산하기 위한 시작 위치만 저장
  void _handlePointerDown(
    PointerDownEvent event,
  ) {
    if (_activePointer != null) {
      return;
    }

    _activePointer = event.pointer;
    _dragStartPosition = event.localPosition;
  }

  // 터치 시작점부터 현재 위치까지의 드래그 거리를 계산
  //
  // thumb는 최대 이동 범위 안에서만 이동하고,
  // 외부 입력에는 방향과 이동 거리에 따른 속도 배율을 저장
  void _handlePointerMove(
    PointerMoveEvent event,
    double maxDistance,
  ) {
    if (_activePointer != event.pointer ||
        _dragStartPosition == null ||
        maxDistance <= 0.0) {
      return;
    }

    final dragOffset = event.localPosition - _dragStartPosition!;

    final Offset limitedOffset;

    if (dragOffset.distance <= maxDistance) {
      limitedOffset = dragOffset;
    } else {
      // 최대 범위를 벗어난 경우 방향은 유지하고
      // 거리를 maxDistance로 제한
      limitedOffset = Offset.fromDirection(
        dragOffset.direction,
        maxDistance,
      );
    }

    // 화면상의 이동량을 -1.0 ~ 1.0으로 정규화
    final normalizedX = limitedOffset.dx / maxDistance;
    final normalizedY = -limitedOffset.dy / maxDistance;

    final axisDirection = Offset(
      _getAxisDirection(normalizedX),
      _getAxisDirection(normalizedY),
    );

    // thumb 이동 거리를 0.0 ~ 1.0 범위로 정규화
    final distanceRatio = (limitedOffset.distance / maxDistance).clamp(
      0.0,
      1.0,
    );

    // 중앙에서는 정밀하게 움직이고,
    // 외곽으로 갈수록 속도가 빠르게 증가
    final speedScale = math
        .pow(
          distanceRatio,
          1.7,
        )
        .toDouble();

    final inputDirection = axisDirection * speedScale;

    setState(() {
      // thumb 표시는 실제 드래그 거리를 사용
      _thumbOffset = limitedOffset;

      // 각 축의 입력 방향과 이동 거리에 따른 속도 배율 저장
      _inputDirection = inputDirection;

      // 적어도 하나의 축에 방향이 있으면 입력 활성화
      _isInputActive = inputDirection != Offset.zero;
    });
  }

  // 현재 조작 중인 포인터가 해제되면 중앙으로 복귀
  void _handlePointerUp(
    PointerUpEvent event,
  ) {
    if (_activePointer != event.pointer) {
      return;
    }

    _resetThumb();
  }

  // 포인터 입력이 취소되면 중앙으로 복귀
  void _handlePointerCancel(
    PointerCancelEvent event,
  ) {
    if (_activePointer != event.pointer) {
      return;
    }

    _resetThumb();
  }

  // 정규화된 축 값을 -1.0, 0.0, 1.0 방향 값으로 변환
  //
  // 중앙 부근의 미세한 움직임은 deadZone으로 무시
  double _getAxisDirection(
    double value,
  ) {
    if (value.abs() < widget.deadZone) {
      return 0.0;
    }

    if (value < 0.0) {
      return -1.0;
    }

    return 1.0;
  }

  // 20ms마다 호출
  //
  // 타이머는 항상 유지되지만 입력이 비활성 상태이면
  // 외부 콜백을 호출하지 않고 즉시 반환
  void _handlePeriodicUpdate() {
    if (!mounted || !_isInputActive || _inputDirection == Offset.zero) {
      return;
    }

    final updateValue = Offset(
      _inputDirection.dx * widget.updateStep,
      _inputDirection.dy * widget.updateStep,
    );

    widget.onUpdate?.call(
      updateValue,
    );
  }

  // thumb와 입력 상태를 중앙 상태로 초기화
  void _resetThumb() {
    setState(() {
      _activePointer = null;
      _dragStartPosition = null;

      _thumbOffset = Offset.zero;
      _inputDirection = Offset.zero;

      _isInputActive = false;
    });

    widget.onReleased?.call();
  }

  Widget _buildDirectionArrow({
    required int quarterTurns,
    required Offset offset,
    required bool isActive,
    required double size,
  }) {
    return IgnorePointer(
      child: Transform.translate(
        offset: offset,
        child: RotatedBox(
          quarterTurns: quarterTurns,
          child: Icon(
            Icons.change_history,
            size: size,
            color: Colors.white.withAlpha(
              isActive ? 210 : 100,
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
        final size = constraints.biggest.shortestSide;

        if (!size.isFinite || size <= 0.0) {
          return const SizedBox.shrink();
        }

        final baseRadius = size / 2.0;

        // thumb 크기는 전체 조이스틱 크기의 38%
        final thumbSize = size * 0.38;
        final thumbRadius = thumbSize / 2.0;

        // thumb 전체가 외곽 원 안에 있도록 최대 이동 거리 계산
        final maxDistance = baseRadius - thumbRadius;

        // thumb 중심 이동 제한선과 외곽 원 사이에 화살표 배치
        final arrowDistance = maxDistance + (baseRadius - maxDistance) * 0.52;

        final arrowSize = size * 0.16;

        return SizedBox.square(
          dimension: size,
          child: ClipOval(
            child: Listener(
              behavior: HitTestBehavior.opaque,
              onPointerDown: _handlePointerDown,
              onPointerMove: (
                PointerMoveEvent event,
              ) {
                _handlePointerMove(
                  event,
                  maxDistance,
                );
              },
              onPointerUp: _handlePointerUp,
              onPointerCancel: _handlePointerCancel,
              child: Container(
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: Colors.black.withAlpha(70),
                  border: Border.all(
                    color: Colors.white.withAlpha(100),
                    width: 2.0,
                  ),
                ),
                child: Stack(
                  alignment: Alignment.center,
                  children: [
                    // 위쪽 화살표
                    _buildDirectionArrow(
                      quarterTurns: 0,
                      offset: Offset(
                        0.0,
                        -arrowDistance,
                      ),
                      isActive: _inputDirection.dy > 0.0,
                      size: arrowSize,
                    ),

                    // 오른쪽 화살표
                    _buildDirectionArrow(
                      quarterTurns: 1,
                      offset: Offset(
                        arrowDistance,
                        0.0,
                      ),
                      isActive: _inputDirection.dx > 0.0,
                      size: arrowSize,
                    ),

                    // 아래쪽 화살표
                    _buildDirectionArrow(
                      quarterTurns: 2,
                      offset: Offset(
                        0.0,
                        arrowDistance,
                      ),
                      isActive: _inputDirection.dy < 0.0,
                      size: arrowSize,
                    ),

                    // 왼쪽 화살표
                    _buildDirectionArrow(
                      quarterTurns: 3,
                      offset: Offset(
                        -arrowDistance,
                        0.0,
                      ),
                      isActive: _inputDirection.dx < 0.0,
                      size: arrowSize,
                    ),

                    // 조이스틱 thumb
                    Transform.translate(
                      offset: _thumbOffset,
                      child: Container(
                        width: thumbSize,
                        height: thumbSize,
                        decoration: BoxDecoration(
                          shape: BoxShape.circle,
                          color: Colors.white.withAlpha(110),
                          border: Border.all(
                            color: Colors.white.withAlpha(160),
                            width: 1.5,
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        );
      },
    );
  }
}
