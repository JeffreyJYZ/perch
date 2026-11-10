// Draws the app icon on Apple's grid, in 1024 px, and writes one PNG.
//
// The plate is 804 of 1024 with the rest transparent, because art that fills its canvas
// renders oversized in the Dock; the corner radius is the macOS ratio of that plate. The
// glyph is one bird on one perch — a dark plate, a dim bar, a light body — so the mark still
// reads at 16 px, where a second colour or an eye would turn to noise.
//
//   swift scripts/make-icon.swift assets/icon-1024.png

import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

let canvas: CGFloat = 1024
let plateSide: CGFloat = 804
let plateInset = (canvas - plateSide) / 2
let plateRadius = plateSide * 0.2237
let plate = CGRect(x: plateInset, y: plateInset, width: plateSide, height: plateSide)

let glyph = CGColor(srgbRed: 0.949, green: 0.953, blue: 0.961, alpha: 1)

func fail(_ why: String) -> Never {
    FileHandle.standardError.write(Data("make-icon: \(why)\n".utf8))
    exit(1)
}

func capsule(_ a: CGPoint, _ b: CGPoint, _ width: CGFloat, _ context: CGContext) {
    context.setLineWidth(width)
    context.setLineCap(.round)
    context.move(to: a)
    context.addLine(to: b)
    context.strokePath()
}

let out = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "assets/icon-1024.png"
guard let space = CGColorSpace(name: CGColorSpace.sRGB) else { fail("no sRGB profile") }
guard
    let context = CGContext(
        data: nil,
        width: Int(canvas),
        height: Int(canvas),
        bitsPerComponent: 8,
        bytesPerRow: 0,
        space: space,
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
    )
else { fail("no bitmap context") }

context.setShouldAntialias(true)
context.setAllowsAntialiasing(true)

// The plate: a vertical gradient, lit from above, with a hairline rim so the edge survives
// against a light desktop as well as a dark one.
let platePath = CGPath(
    roundedRect: plate,
    cornerWidth: plateRadius,
    cornerHeight: plateRadius,
    transform: nil
)
context.saveGState()
context.addPath(platePath)
context.clip()
let shade = CGGradient(
    colorsSpace: space,
    colors: [
        CGColor(srgbRed: 0.141, green: 0.153, blue: 0.180, alpha: 1),
        CGColor(srgbRed: 0.047, green: 0.055, blue: 0.067, alpha: 1),
    ] as CFArray,
    locations: [0, 1]
)
guard let shade else { fail("no gradient") }
context.drawLinearGradient(
    shade,
    start: CGPoint(x: plate.midX, y: plate.maxY),
    end: CGPoint(x: plate.midX, y: plate.minY),
    options: []
)
context.restoreGState()

context.setStrokeColor(CGColor(srgbRed: 1, green: 1, blue: 1, alpha: 0.07))
context.setLineWidth(2)
context.addPath(
    CGPath(
        roundedRect: plate.insetBy(dx: 1.5, dy: 1.5),
        cornerWidth: plateRadius - 1.5,
        cornerHeight: plateRadius - 1.5,
        transform: nil
    )
)
context.strokePath()

// Glyph geometry, all as a fraction of the plate so the mark keeps its proportion.
let barFrom = CGPoint(x: plate.minX + plateSide * 0.20, y: plate.minY + plateSide * 0.400)
let barTo = CGPoint(x: plate.minX + plateSide * 0.80, y: plate.minY + plateSide * 0.400)
let barWidth = plateSide * 0.031

let body = CGPoint(x: plate.minX + plateSide * 0.425, y: barFrom.y + barWidth + plateSide * 0.088)
let bodyRadius = plateSide * 0.088

// Tail first, so the perch crosses in front of it: a bird sits in front of the branch, its tail
// behind. Everything else is a filled shape over the bar.
context.setStrokeColor(glyph.copy(alpha: 0.92) ?? glyph)
capsule(body, CGPoint(x: body.x - plateSide * 0.072, y: body.y - plateSide * 0.154), plateSide * 0.030, context)

context.setStrokeColor(glyph.copy(alpha: 0.45) ?? glyph)
capsule(barFrom, barTo, barWidth, context)

context.setFillColor(glyph)
context.fillEllipse(
    in: CGRect(
        x: body.x - bodyRadius,
        y: body.y - bodyRadius,
        width: bodyRadius * 2,
        height: bodyRadius * 2
    )
)

// The beak: a small wedge off the right of the body, tilted a little down so it reads as a beak
// rather than a speech-bubble tail, and narrow enough to keep the mark quiet at 16 px.
let beak = CGMutablePath()
let beakSpread = bodyRadius * 0.23
let beakReach = bodyRadius * 1.40
beak.move(to: CGPoint(x: body.x + bodyRadius * 0.95, y: body.y + beakSpread))
beak.addLine(to: CGPoint(x: body.x + bodyRadius * 0.95, y: body.y - beakSpread))
beak.addLine(to: CGPoint(x: body.x + beakReach, y: body.y - bodyRadius * 0.12))
beak.closeSubpath()
context.addPath(beak)
context.fillPath()

guard let image = context.makeImage() else { fail("no image") }
guard
    let destination = CGImageDestinationCreateWithURL(
        URL(fileURLWithPath: out) as CFURL,
        UTType.png.identifier as CFString,
        1,
        nil
    )
else { fail("cannot write \(out)") }
CGImageDestinationAddImage(destination, image, nil)
guard CGImageDestinationFinalize(destination) else { fail("cannot finish \(out)") }
print("make-icon: wrote \(out) (\(Int(canvas)) px)")
