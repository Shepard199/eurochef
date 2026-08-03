# EuroChef Robots Fork — CLI FAQ

Run commands from this directory with `cargo run -p eurochef-cli --` during development, or use the built `eurochef-cli.exe`.

## What commands exist?

```text
eurochef-cli filelist extract|create
eurochef-cli edb entities|spreadsheets|maps|textures|animations
eurochef-cli edb particle-report|script-health|trigger-report|entity-report|anim-binding-report
```

Use `--help` at every level for the authoritative argument list:

```powershell
cargo run -p eurochef-cli -- --help
cargo run -p eurochef-cli -- edb maps --help
```

## How do I extract a Robots map?

```powershell
cargo run -p eurochef-cli -- edb maps D:\Games\Robots\_eurotools_out\extracted_main\robots\binary\_bin_pc\m01_vill.edb out --platform pc
```

## How do I export entities or textures?

```powershell
cargo run -p eurochef-cli -- edb entities input.edb out --platform pc
cargo run -p eurochef-cli -- edb textures input.edb out --platform pc --format png
```

## How do I inspect corpus health?

```powershell
cargo run -p eurochef-cli -- edb script-health D:\Games\Robots\_eurotools_out\edb\manifest.tsv out
cargo run -p eurochef-cli -- edb trigger-report D:\Games\Robots\_eurotools_out\edb\manifest.tsv out
```

## How do I run the GUI?

Use `RUN_GUI.cmd`. The GUI has its own native PC MUSX decoder and discovers sound data from the opened EDB path or `EUROSOUND_PROJECT_ROOT`; no EuroSoundBridge executable is required.
