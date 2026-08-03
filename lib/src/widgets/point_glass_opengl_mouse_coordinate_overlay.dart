import 'package:flutter/material.dart';

import 'package:vector_math/vector_math.dart' as vm;

class PointGlassOpenGLMouseCoordinateOverlay extends StatelessWidget {
  const PointGlassOpenGLMouseCoordinateOverlay({
    super.key,
    required this.screenPosition,
    required this.worldPosition,
    required this.viewerSize,
  });

  final Offset screenPosition;
  final vm.Vector3? worldPosition;
  final Size viewerSize;

  @override
  Widget build(BuildContext context) {
    const double margin = 12.0;
    const double overlayWidth = 100.0;
    const double overlayHeight = 50.0;
    const EdgeInsetsGeometry padding =
        EdgeInsets.symmetric(horizontal: 8, vertical: 6);
    const Decoration decoration = BoxDecoration(
      color: Color(0x64000000),
      borderRadius: BorderRadius.all(Radius.circular(4)),
      border: Border.fromBorderSide(BorderSide(color: Color(0x64ffffff))),
    );
    const TextStyle textStyle = TextStyle(
      color: Colors.white,
      fontSize: 11,
    );

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
        : 'X: ${(worldPosition!.x).toStringAsFixed(2)} m\n'
            'Y: ${worldPosition!.y.toStringAsFixed(2)} m';

    return Positioned(
      left: left,
      top: top,
      width: overlayWidth,
      child: IgnorePointer(
        child: Container(
          padding: padding,
          decoration: decoration,
          child: Text(
            coordinateText,
            style: textStyle,
          ),
        ),
      ),
    );
  }
}
