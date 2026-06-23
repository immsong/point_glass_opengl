import 'package:flutter/material.dart';
import 'package:vector_math/vector_math.dart' as vm;

class PointGlassOpenGLPoints {
  final bool enable;
  final List<vm.Vector3> points;
  final double pointSize;
  final Color color;

  PointGlassOpenGLPoints({
    this.enable = true,
    required this.points,
    this.pointSize = 3.0,
    this.color = Colors.white,
  });
}
