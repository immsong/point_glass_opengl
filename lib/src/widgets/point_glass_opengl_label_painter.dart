import 'package:flutter/material.dart';

import 'package:vector_math/vector_math.dart' as vm;

import 'package:point_glass_opengl/src/models/point_glass_opengl_label.dart';
import 'package:point_glass_opengl/src/core/point_glass_opengl_controller.dart';

class PointGlassOpenGLLabelPainter extends CustomPainter {
  final PointGlassOpenGLController controller;
  final List<PointGlassOpenGLLabel> labels;

  PointGlassOpenGLLabelPainter({
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
    covariant PointGlassOpenGLLabelPainter oldDelegate,
  ) {
    return oldDelegate.labels != labels || oldDelegate.controller != controller;
  }
}
