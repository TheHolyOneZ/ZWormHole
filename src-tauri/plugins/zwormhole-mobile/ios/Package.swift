// swift-tools-version:5.3

import PackageDescription

let package = Package(
  name: "tauri-plugin-zwormhole-mobile",
  platforms: [
    .macOS(.v10_13),
    .iOS(.v13),
  ],
  products: [
    .library(
      name: "tauri-plugin-zwormhole-mobile",
      type: .static,
      targets: ["tauri-plugin-zwormhole-mobile"])
  ],
  dependencies: [
    .package(name: "Tauri", path: "../.tauri/tauri-api")
  ],
  targets: [
    .target(
      name: "tauri-plugin-zwormhole-mobile",
      dependencies: [
        .byName(name: "Tauri")
      ],
      path: "Sources")
  ]
)
