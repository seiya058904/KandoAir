// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

fn main() {
    println!("cargo:rerun-if-changed=resources/app.rc");
    println!("cargo:rerun-if-changed=resources/kandoair.manifest");
    println!("cargo:rerun-if-changed=../assets/icons/icon.ico");
    embed_resource::compile("resources/app.rc", embed_resource::NONE)
        .manifest_required()
        .expect("failed to compile Windows resources");
}
