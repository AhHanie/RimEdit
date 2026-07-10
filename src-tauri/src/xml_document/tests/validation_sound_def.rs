use super::*;

// --- SoundDef alias validation ---

#[test]
fn sound_def_accepts_sustainer_aliases() {
    // XML using legacy [LoadAlias] names (sustainerStartSound, sustainerStopSound,
    // sustainerFadeoutStartSound) must not produce validation_unknown_field warnings.
    let src = r#"<Defs>
  <SoundDef>
    <defName>MySustainer</defName>
    <sustain>true</sustain>
    <sustainerStartSound>OtherSound</sustainerStartSound>
    <sustainerStopSound>OtherSoundStop</sustainerStopSound>
    <sustainerFadeoutStartSound>OtherSoundFadeout</sustainerFadeoutStartSound>
    <subSounds>
      <li>
        <grains>
          <li Class="AudioGrain_Folder">
            <clipFolderPath>Sounds/Ambient</clipFolderPath>
          </li>
        </grains>
        <volumeRange>0.5~1.0</volumeRange>
        <sustainLoop>true</sustainLoop>
      </li>
    </subSounds>
  </SoundDef>
</Defs>"#;

    let diagnostics = validate_test_xml(src, &empty_def_index());

    let alias_warnings: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            d.code == "validation_unknown_field"
                && (d.field_path.as_deref() == Some("sustainerStartSound")
                    || d.field_path.as_deref() == Some("sustainerStopSound")
                    || d.field_path.as_deref() == Some("sustainerFadeoutStartSound"))
        })
        .collect();

    assert!(
        alias_warnings.is_empty(),
        "Alias names must not produce validation_unknown_field; got: {:#?}",
        alias_warnings
    );
}

#[test]
fn sound_def_accepts_sustain_interval_alias() {
    // XML using the sustainInterval alias for sustainIntervalRange must not produce
    // validation_unknown_object_field inside the subSounds object list.
    let src = r#"<Defs>
  <SoundDef>
    <defName>IntervalSound</defName>
    <subSounds>
      <li>
        <grains>
          <li Class="AudioGrain_Clip">
            <clipPath>Sounds/Click</clipPath>
          </li>
        </grains>
        <volumeRange>0.8~1.0</volumeRange>
        <sustainInterval>1.0~2.0</sustainInterval>
      </li>
    </subSounds>
  </SoundDef>
</Defs>"#;

    let diagnostics = validate_test_xml(src, &empty_def_index());

    let alias_warnings: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            d.code == "validation_unknown_object_field"
                && d.field_path
                    .as_deref()
                    .map(|p| p.contains("sustainInterval"))
                    .unwrap_or(false)
        })
        .collect();

    assert!(
        alias_warnings.is_empty(),
        "sustainInterval alias must not produce unknown field warnings; got: {:#?}",
        alias_warnings
    );
}

// --- SoundDef audio grain variant validation ---

#[test]
fn sound_def_validates_audio_grain_variants() {
    let src = r#"<Defs>
  <SoundDef>
    <defName>GrainTest</defName>
    <subSounds>
      <li>
        <grains>
          <li Class="AudioGrain_Clip">
            <clipPath>Sounds/Gunshot/Shot</clipPath>
          </li>
          <li Class="AudioGrain_Folder">
            <clipFolderPath>Sounds/Gunshot</clipFolderPath>
          </li>
        </grains>
        <volumeRange>0.8~1.0</volumeRange>
        <pitchRange>0.9~1.1</pitchRange>
      </li>
    </subSounds>
  </SoundDef>
</Defs>"#;

    let diagnostics = validate_test_xml(src, &empty_def_index());

    // No unknown-field warnings for grain variant fields.
    let grain_unknowns: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            (d.code == "validation_unknown_field" || d.code == "validation_unknown_object_field")
                && d.field_path
                    .as_deref()
                    .map(|p| p.contains("clipPath") || p.contains("clipFolderPath"))
                    .unwrap_or(false)
        })
        .collect();

    assert!(
        grain_unknowns.is_empty(),
        "Audio grain variant fields must be schema-backed; got: {:#?}",
        grain_unknowns
    );
}

// --- SoundDef parameter mapping validation ---

#[test]
fn sound_def_validates_nested_param_mapping_objects() {
    let src = r#"<Defs>
  <SoundDef>
    <defName>MappedSound</defName>
    <subSounds>
      <li>
        <grains>
          <li Class="AudioGrain_Folder">
            <clipFolderPath>Sounds/Ambient</clipFolderPath>
          </li>
        </grains>
        <volumeRange>0.1~1.0</volumeRange>
        <sustainLoop>true</sustainLoop>
        <paramMappings>
          <li>
            <inParam Class="SoundParamSource_External">
              <inParamName>Volume</inParamName>
            </inParam>
            <outParam Class="SoundParamTarget_Volume"/>
            <paramUpdateMode>Constant</paramUpdateMode>
          </li>
        </paramMappings>
      </li>
    </subSounds>
  </SoundDef>
</Defs>"#;

    let diagnostics = validate_test_xml(src, &empty_def_index());

    // No unknown-field warnings for paramMappings content.
    let mapping_unknowns: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            (d.code == "validation_unknown_field" || d.code == "validation_unknown_object_field")
                && d.field_path
                    .as_deref()
                    .map(|p| {
                        p.contains("paramMappings")
                            || p.contains("inParam")
                            || p.contains("outParam")
                            || p.contains("inParamName")
                            || p.contains("paramUpdateMode")
                    })
                    .unwrap_or(false)
        })
        .collect();

    assert!(
        mapping_unknowns.is_empty(),
        "paramMappings, inParam, outParam fields must be schema-backed; got: {:#?}",
        mapping_unknowns
    );
}

// --- SoundDef SimpleCurve validation ---

#[test]
fn sound_def_validates_simple_curve_points() {
    let src = r#"<Defs>
  <SoundDef>
    <defName>CurveSound</defName>
    <subSounds>
      <li>
        <grains>
          <li Class="AudioGrain_Folder">
            <clipFolderPath>Sounds/Ambient</clipFolderPath>
          </li>
        </grains>
        <volumeRange>0.1~1.0</volumeRange>
        <paramMappings>
          <li>
            <inParam Class="SoundParamSource_CameraAltitude"/>
            <outParam Class="SoundParamTarget_Volume"/>
            <curve>
              <points>
                <li>(0, 0)</li>
                <li>(1, 1)</li>
              </points>
            </curve>
          </li>
        </paramMappings>
      </li>
    </subSounds>
  </SoundDef>
</Defs>"#;

    let diagnostics = validate_test_xml(src, &empty_def_index());

    // The curve field must be schema-backed - no unknown-field warning for it.
    let curve_unknowns: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            (d.code == "validation_unknown_field" || d.code == "validation_unknown_object_field")
                && d.field_path
                    .as_deref()
                    .map(|p| p.ends_with("curve") || p.contains("curve."))
                    .unwrap_or(false)
        })
        .collect();

    assert!(
        curve_unknowns.is_empty(),
        "curve field must be schema-backed; got: {:#?}",
        curve_unknowns
    );

    // The points field inside SimpleCurve must also be schema-backed.
    let points_unknowns: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            (d.code == "validation_unknown_field" || d.code == "validation_unknown_object_field")
                && d.field_path
                    .as_deref()
                    .map(|p| p.ends_with("points") || p.contains("points"))
                    .unwrap_or(false)
        })
        .collect();

    assert!(
        points_unknowns.is_empty(),
        "curve.points must be schema-backed; got: {:#?}",
        points_unknowns
    );
}
