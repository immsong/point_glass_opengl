import 'package:flutter/material.dart';

import 'package:point_glass_opengl/src/widgets/point_glass_opengl_joystick.dart';

class PointGlassOpenGLDualJoystick extends StatelessWidget {
  final Size viewerSize;
  final ValueChanged<Offset>? onUpdateLeft;
  final VoidCallback? onReleasedLeft;
  final double updateStepLeft;
  final ValueChanged<Offset>? onUpdateRight;
  final VoidCallback? onReleasedRight;
  final double updateStepRight;

  const PointGlassOpenGLDualJoystick({
    super.key,
    required this.viewerSize,
    this.onUpdateLeft,
    this.onReleasedLeft,
    this.updateStepLeft = 1.0,
    this.onUpdateRight,
    this.onReleasedRight,
    this.updateStepRight = 1.0,
  });

  @override
  Widget build(BuildContext context) {
    final joystickSize =
        (viewerSize.shortestSide * 0.16).clamp(80.0, 140.0).toDouble();

    final margin =
        (viewerSize.shortestSide * 0.04).clamp(16.0, 32.0).toDouble();

    return Stack(
      children: [
        Positioned(
          left: margin,
          bottom: margin,
          width: joystickSize,
          height: joystickSize,
          child: PointGlassOpenGLJoystick(
            onUpdate: onUpdateLeft,
            onReleased: onReleasedLeft,
            updateStep: updateStepLeft,
          ),
        ),
        Positioned(
          right: margin,
          bottom: margin,
          width: joystickSize,
          height: joystickSize,
          child: PointGlassOpenGLJoystick(
            onUpdate: onUpdateRight,
            onReleased: onReleasedRight,
            updateStep: updateStepRight,
          ),
        ),
      ],
    );
  }
}
