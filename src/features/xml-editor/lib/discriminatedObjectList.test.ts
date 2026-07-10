/**
 * TestActionDef schema descriptor tests.
 *
 * Verifies that TestActionDef direct fields and ActionComponent comp
 * variants produce schema-backed descriptors rather than readonlyUnknown.
 */
import { buildFormDescriptors } from "./formDescriptors";
import type { DefEditorView, XmlChildView, XmlListItemView } from "../types/xmlDocument";
import type { FieldSchema, ObjectTypeSchema, SchemaCatalog } from "../../schema-catalog";

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

function makeField(overrides: Partial<FieldSchema> = {}): FieldSchema {
  return {
    type: { kind: "string" },
    required: false,
    repeatable: false,
    xml: "element",
    examples: [],
    flags: false,
    ...overrides,
  };
}

function makeDef(
  defType: string,
  children: XmlChildView[],
): DefEditorView {
  return {
    nodeId: 0,
    defType,
    defName: "TestAction",
    label: null,
    parentName: null,
    line: null,
    column: null,
    attributes: [],
    children,
  };
}

function makeChild(
  name: string,
  textValue: string | null = null,
  xmlShape: XmlChildView["xmlShape"] = "element",
): XmlChildView {
  return {
    nodeId: 1,
    name,
    textValue,
    listItems: [],
    xmlShape,
    order: 0,
    known: false,
    line: null,
    column: null,
  };
}

function makeListItemChild(
  name: string,
  liItems: XmlListItemView[],
): XmlChildView {
  return {
    nodeId: 1,
    name,
    textValue: null,
    listItems: [],
    xmlShape: "listOfLi",
    order: 0,
    known: false,
    line: null,
    column: null,
    liItems,
  };
}

function makeListItem(
  className: string,
  children: { name: string; value: string }[],
): XmlListItemView {
  return {
    nodeId: 2,
    textValue: null,
    attributes: className ? [{ name: "Class", value: className, known: true }] : [],
    children: children.map((c) => ({
      nodeId: 3,
      name: c.name,
      textValue: c.value,
      listItems: [],
      xmlShape: "element",
      order: 0,
      line: null,
      column: null,
    })),
    order: 0,
    line: null,
    column: null,
    selfClosing: false,
  };
}

/** TestActionDef schema fields (subset needed for tests) */
const actionDefFields: Record<string, FieldSchema> = {
  tier: makeField({ type: { kind: "integer" }, defaultValue: 0 }),
  iconPath: makeField({ type: { kind: "string" } }),
  isHostile: makeField({ type: { kind: "boolean" }, defaultValue: true }),
  requiresCapability: makeField({ type: { kind: "boolean" }, defaultValue: true }),
  statModifiers: makeField({ type: { kind: "statMap" }, xml: "namedChildrenMap" }),
  actionVerb: makeField({ type: { kind: "object", schemaRef: "ActionVerb" }, xml: "object" }),
  components: makeField({
    type: { kind: "list" },
    xml: "listOfLi",
    items: { kind: "object", schemaRef: "ActionComponent" },
  }),
  cooldownRange: makeField({ type: { kind: "intRange" } }),
  requiredTags: makeField({
    type: { kind: "defReference" },
    xml: "listOfLi",
    reference: { defType: "TagDef", allowAbstract: false, scope: "allSources" },
  }),
  showWhilePassive: makeField({ type: { kind: "boolean" }, defaultValue: false }),
  disableWhilePassive: makeField({ type: { kind: "boolean" }, defaultValue: true }),
  logOnUse: makeField({ type: { kind: "boolean" }, defaultValue: false }),
  maxUses: makeField({ type: { kind: "integer" }, defaultValue: -1 }),
  keyBinding: makeField({
    type: { kind: "defReference" },
    reference: { defType: "HotkeyDef", allowAbstract: false, scope: "allSources" },
  }),
  showWhenActive: makeField({ type: { kind: "boolean" }, defaultValue: true }),
  showOnProfile: makeField({ type: { kind: "boolean" }, defaultValue: true }),
  stunOnCast: makeField({ type: { kind: "boolean" }, defaultValue: false }),
};

const COMPONENT_BASE_FIELDS: Record<string, FieldSchema> = {
  Class: makeField({ type: { kind: "typeName" }, xml: "attribute" }),
  componentClass: makeField({ type: { kind: "typeName" } }),
  reputationImpact: makeField({ type: { kind: "integer" }, defaultValue: 0 }),
  isPsychic: makeField({ type: { kind: "boolean" }, defaultValue: false }),
  canTargetMechs: makeField({ type: { kind: "boolean" }, defaultValue: true }),
  applyReputationToLodgers: makeField({ type: { kind: "boolean" }, defaultValue: true }),
  canTargetElites: makeField({ type: { kind: "boolean" }, defaultValue: true }),
  activationSound: makeField({
    type: { kind: "defReference" },
    reference: { defType: "SoundTemplate", allowAbstract: false, scope: "allSources" },
  }),
  sendNotification: makeField({ type: { kind: "boolean" }, defaultValue: true }),
  selectionWeight: makeField({ type: { kind: "float" }, defaultValue: 1 }),
  allowWoundedTarget: makeField({ type: { kind: "boolean" }, defaultValue: true }),
  canTargetMinors: makeField({ type: { kind: "boolean" }, defaultValue: true }),
};

function makeActionComponentSchema(
  extraVariants: Record<string, string> = {},
): ObjectTypeSchema {
  return {
    fieldOrder: ["Class", "componentClass"],
    fields: {
      Class: makeField({ type: { kind: "typeName" }, xml: "attribute" }),
      componentClass: makeField({ type: { kind: "typeName" } }),
    },
    discriminator: {
      attribute: "Class",
      fallbackSchemaRef: "ActionComponent",
      allowMissing: true,
      allowUnknown: true,
      variants: {
        ActionComponent_SpawnEffect: "ActionComponent_SpawnEffect",
        ActionComponent_AddStatus: "ActionComponent_AddStatus",
        ActionComponent_SocialInteract: "ActionComponent_SocialInteract",
        ActionComponent_AttachEffect: "ActionComponent_AttachEffect",
        ActionComponent_Projectile: "ActionComponent_Projectile",
        ActionComponent_Spray: "ActionComponent_Spray",
        ActionComponent_ResourceCost: "ActionComponent_ResourceCost",
        ActionComponent_RequiresCapability: "ActionComponent_RequiresCapability",
        ActionComponent_ApplyMood: "ActionComponent_ApplyMood",
        ActionComponent_Teleport: "ActionComponent_Teleport",
        ...extraVariants,
      },
    },
  };
}

function makeActionVerbSchema(): ObjectTypeSchema {
  return {
    fieldOrder: ["warmupTime", "range"],
    fields: {
      warmupTime: makeField({ type: { kind: "float" } }),
      range: makeField({ type: { kind: "float" } }),
    },
  };
}

function makeCatalog(objectTypes: Record<string, ObjectTypeSchema> = {}): SchemaCatalog {
  return {
    formatVersion: 1,
    packs: [],
    defTypes: {
      Def: {
        inherits: [],
        abstractType: true,
        fieldOrder: ["defName", "label", "description"],
        fields: {
          defName: makeField({ type: { kind: "string" }, required: true }),
          label: makeField({ type: { kind: "localizedString" } }),
          description: makeField({ type: { kind: "localizedString" } }),
        },
      },
      TestActionDef: {
        inherits: ["Def"],
        abstractType: false,
        fieldOrder: Object.keys(actionDefFields),
        fields: actionDefFields,
      },
    },
    objectTypes: {
      ActionVerb: makeActionVerbSchema(),
      ActionComponent: makeActionComponentSchema(),
      ...objectTypes,
    },
  };
}

// ---------------------------------------------------------------------------
// 1. Direct TestActionDef field descriptor tests
// ---------------------------------------------------------------------------

describe("TestActionDef direct field descriptors", () => {
  it("tier maps to number control (schema-backed)", () => {
    const catalog = makeCatalog();
    const schema = catalog.defTypes["TestActionDef"]!;
    const def = makeDef("TestActionDef", [makeChild("tier", "3")]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "tier");
    expect(d).toBeDefined();
    expect(d!.control).toBe("number");
    expect(d!.readonly).toBe(false);
    expect(d!.value).toBe("3");
  });

  it("isHostile maps to checkbox control (schema-backed)", () => {
    const catalog = makeCatalog();
    const schema = catalog.defTypes["TestActionDef"]!;
    const def = makeDef("TestActionDef", [makeChild("isHostile", "false")]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "isHostile");
    expect(d).toBeDefined();
    expect(d!.control).toBe("checkbox");
    expect(d!.readonly).toBe(false);
  });

  it("cooldownRange maps to text control (intRange kind)", () => {
    const catalog = makeCatalog();
    const schema = catalog.defTypes["TestActionDef"]!;
    const def = makeDef("TestActionDef", [makeChild("cooldownRange", "60000~60000")]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "cooldownRange");
    expect(d).toBeDefined();
    expect(d!.control).toBe("text");
    expect(d!.readonly).toBe(false);
    expect(d!.value).toBe("60000~60000");
  });

  it("statModifiers maps to namedMap control (schema-backed)", () => {
    const catalog = makeCatalog();
    const schema = catalog.defTypes["TestActionDef"]!;
    const def = makeDef("TestActionDef", [makeChild("statModifiers", null, "namedChildrenMap")]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "statModifiers");
    expect(d).toBeDefined();
    expect(d!.control).toBe("namedMap");
    expect(d!.readonly).toBe(false);
  });

  it("requiredTags maps to list control (listOfLi defReference)", () => {
    const catalog = makeCatalog();
    const schema = catalog.defTypes["TestActionDef"]!;
    const def = makeDef("TestActionDef", [makeChild("requiredTags", null, "listOfLi")]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "requiredTags");
    expect(d).toBeDefined();
    expect(d!.control).toBe("list");
    expect(d!.readonly).toBe(false);
    expect(d!.reference?.defType).toBe("TagDef");
  });

  it("components maps to objectList control with ActionComponent schemaRef", () => {
    const catalog = makeCatalog();
    const schema = catalog.defTypes["TestActionDef"]!;
    const def = makeDef("TestActionDef", [makeChild("components", null, "listOfLi")]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "components");
    expect(d).toBeDefined();
    expect(d!.control).toBe("objectList");
    expect(d!.itemSchemaRef).toBe("ActionComponent");
  });

  it("actionVerb expands to nested ActionVerb fields", () => {
    const catalog = makeCatalog();
    const schema = catalog.defTypes["TestActionDef"]!;
    const def = makeDef("TestActionDef", [makeChild("actionVerb", null, "object")]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const warmup = descs.find((x) => x.key === "actionVerb.warmupTime");
    expect(warmup).toBeDefined();
    expect(warmup!.control).toBe("number");
    expect(warmup!.readonly).toBe(false);
  });

  it("does NOT produce readonlyUnknown for any of the known direct TestActionDef fields", () => {
    const catalog = makeCatalog();
    const schema = catalog.defTypes["TestActionDef"]!;
    const def = makeDef("TestActionDef", [
      makeChild("tier", "3"),
      makeChild("iconPath", "UI/Actions/TestAction"),
      makeChild("isHostile", "true"),
      makeChild("requiresCapability", "true"),
      makeChild("cooldownRange", "60000~60000"),
      makeChild("showWhilePassive", "false"),
      makeChild("disableWhilePassive", "true"),
      makeChild("logOnUse", "false"),
      makeChild("maxUses", "-1"),
      makeChild("showWhenActive", "true"),
      makeChild("showOnProfile", "true"),
      makeChild("stunOnCast", "false"),
    ]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const unknowns = descs.filter((d) => d.control === "readonlyUnknown");
    expect(unknowns).toHaveLength(0);
  });

  it("inherits defName and label from Def base schema", () => {
    const catalog = makeCatalog();
    const schema = catalog.defTypes["TestActionDef"]!;
    const def = makeDef("TestActionDef", [
      makeChild("defName", "TestAbilityA"),
      makeChild("label", "test ability a"),
    ]);
    const descs = buildFormDescriptors(def, schema, catalog);
    expect(descs.find((d) => d.key === "defName")).toBeDefined();
    expect(descs.find((d) => d.key === "label")).toBeDefined();
    const unknowns = descs.filter((d) => d.control === "readonlyUnknown");
    expect(unknowns).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// 2. Component variant descriptor tests
// ---------------------------------------------------------------------------

describe("TestActionDef component variant descriptors", () => {
  function makeCompCatalog(variantName: string, variantSchema: ObjectTypeSchema): SchemaCatalog {
    return makeCatalog({ [variantName]: variantSchema });
  }

  function makeVariantSchema(extraFields: Record<string, FieldSchema>): ObjectTypeSchema {
    return {
      fieldOrder: ["Class", "componentClass", ...Object.keys(COMPONENT_BASE_FIELDS).filter((k) => k !== "Class" && k !== "componentClass"), ...Object.keys(extraFields)],
      fields: { ...COMPONENT_BASE_FIELDS, ...extraFields },
    };
  }

  function getComponentsDescriptor(catalog: SchemaCatalog, liItems: XmlListItemView[]) {
    const schema = catalog.defTypes["TestActionDef"]!;
    const def = makeDef("TestActionDef", [makeListItemChild("components", liItems)]);
    const descs = buildFormDescriptors(def, schema, catalog);
    return descs.find((x) => x.key === "components");
  }

  it("ActionComponent_SpawnEffect resolves to variant schema with effectTemplate field", () => {
    const variant = makeVariantSchema({
      effectTemplate: makeField({
        type: { kind: "defReference" },
        reference: { defType: "VisualEffect", allowAbstract: false, scope: "allSources" },
      }),
    });
    const catalog = makeCompCatalog("ActionComponent_SpawnEffect", variant);
    const item = makeListItem("ActionComponent_SpawnEffect", [
      { name: "effectTemplate", value: "Fleck_PsycastAoeLimit" },
    ]);
    const compsDesc = getComponentsDescriptor(catalog, [item]);
    expect(compsDesc).toBeDefined();
    expect(compsDesc!.control).toBe("objectList");
    const value = compsDesc!.value as { kind: "objectList"; items: { className: string; schemaRef: string | null; fields: Record<string, unknown> }[] };
    expect(value.kind).toBe("objectList");
    expect(value.items).toHaveLength(1);
    expect(value.items[0].className).toBe("ActionComponent_SpawnEffect");
    expect(value.items[0].schemaRef).toBe("ActionComponent_SpawnEffect");
    expect("effectTemplate" in value.items[0].fields).toBe(true);
  });

  it("ActionComponent_AddStatus resolves and exposes statusEffect, effectSeverity, overrideExisting", () => {
    const variant = makeVariantSchema({
      durationMultiplier: makeField({
        type: { kind: "defReference" },
        reference: { defType: "StatDef", allowAbstract: false, scope: "allSources" },
      }),
      durationRange: makeField({ type: { kind: "floatRange" }, defaultValue: "0~0" }),
      statusEffect: makeField({
        type: { kind: "defReference" },
        reference: { defType: "StatusEffect", allowAbstract: false, scope: "allSources" },
      }),
      brainOnly: makeField({ type: { kind: "boolean" }, defaultValue: false }),
      affectsCaster: makeField({ type: { kind: "boolean" }, defaultValue: false }),
      casterOnly: makeField({ type: { kind: "boolean" }, defaultValue: false }),
      affectsTarget: makeField({ type: { kind: "boolean" }, defaultValue: true }),
      overrideExisting: makeField({ type: { kind: "boolean" }, defaultValue: false }),
      effectSeverity: makeField({ type: { kind: "float" }, defaultValue: -1 }),
      excludeCaster: makeField({ type: { kind: "boolean" }, defaultValue: false }),
    });
    const catalog = makeCompCatalog("ActionComponent_AddStatus", variant);
    const item = makeListItem("ActionComponent_AddStatus", [
      { name: "statusEffect", value: "PainBlock" },
      { name: "effectSeverity", value: "1" },
      { name: "overrideExisting", value: "true" },
    ]);
    const compsDesc = getComponentsDescriptor(catalog, [item]);
    const value = compsDesc!.value as { kind: "objectList"; items: { schemaRef: string | null; fields: Record<string, unknown> }[] };
    expect(value.items[0].schemaRef).toBe("ActionComponent_AddStatus");
    expect("statusEffect" in value.items[0].fields).toBe(true);
    expect("effectSeverity" in value.items[0].fields).toBe(true);
    expect("overrideExisting" in value.items[0].fields).toBe(true);
  });

  it("ActionComponent_SocialInteract resolves with behaviorTemplate field", () => {
    const variant = makeVariantSchema({
      behaviorTemplate: makeField({
        type: { kind: "defReference" },
        reference: { defType: "BehaviorTemplate", allowAbstract: false, scope: "allSources" },
      }),
      allowMentalBreak: makeField({ type: { kind: "boolean" }, defaultValue: false }),
      allowUnconscious: makeField({ type: { kind: "boolean" }, defaultValue: false }),
      allowAsleep: makeField({ type: { kind: "boolean" }, defaultValue: false }),
    });
    const catalog = makeCompCatalog("ActionComponent_SocialInteract", variant);
    const item = makeListItem("ActionComponent_SocialInteract", [
      { name: "behaviorTemplate", value: "Counsel_Success" },
    ]);
    const compsDesc = getComponentsDescriptor(catalog, [item]);
    const value = compsDesc!.value as { kind: "objectList"; items: { schemaRef: string | null; fields: Record<string, unknown> }[] };
    expect(value.items[0].schemaRef).toBe("ActionComponent_SocialInteract");
    expect("behaviorTemplate" in value.items[0].fields).toBe(true);
  });

  it("ActionComponent_ResourceCost resolves with resourceCost field", () => {
    const variant = makeVariantSchema({
      resourceCost: makeField({ type: { kind: "float" }, defaultValue: 0 }),
    });
    const catalog = makeCompCatalog("ActionComponent_ResourceCost", variant);
    const item = makeListItem("ActionComponent_ResourceCost", [
      { name: "resourceCost", value: "0.3" },
    ]);
    const compsDesc = getComponentsDescriptor(catalog, [item]);
    const value = compsDesc!.value as { kind: "objectList"; items: { schemaRef: string | null; fields: Record<string, unknown> }[] };
    expect(value.items[0].schemaRef).toBe("ActionComponent_ResourceCost");
    expect("resourceCost" in value.items[0].fields).toBe(true);
  });

  it("ActionComponent_RequiresCapability resolves with capabilityTags field (flagsText xml)", () => {
    const variant = makeVariantSchema({
      capabilityTags: makeField({
        type: { kind: "string" },
        xml: "flagsText",
        validationHints: { allowedValues: ["Violent", "Social", "Intellectual"] },
      }),
    });
    const catalog = makeCompCatalog("ActionComponent_RequiresCapability", variant);
    const item = makeListItem("ActionComponent_RequiresCapability", [
      { name: "capabilityTags", value: "Violent" },
    ]);
    const compsDesc = getComponentsDescriptor(catalog, [item]);
    const value = compsDesc!.value as { kind: "objectList"; items: { schemaRef: string | null; fields: Record<string, unknown> }[] };
    expect(value.items[0].schemaRef).toBe("ActionComponent_RequiresCapability");
    expect("capabilityTags" in value.items[0].fields).toBe(true);
  });

  it("ActionComponent_ApplyMood resolves with moodTemplate field", () => {
    const variant = makeVariantSchema({
      moodTemplate: makeField({
        type: { kind: "defReference" },
        reference: { defType: "MoodTemplate", allowAbstract: false, scope: "allSources" },
      }),
      moodTemplateForMechs: makeField({
        type: { kind: "defReference" },
        reference: { defType: "MoodTemplate", allowAbstract: false, scope: "allSources" },
      }),
    });
    const catalog = makeCompCatalog("ActionComponent_ApplyMood", variant);
    const item = makeListItem("ActionComponent_ApplyMood", [
      { name: "moodTemplate", value: "Berserk" },
    ]);
    const compsDesc = getComponentsDescriptor(catalog, [item]);
    const value = compsDesc!.value as { kind: "objectList"; items: { schemaRef: string | null; fields: Record<string, unknown> }[] };
    expect(value.items[0].schemaRef).toBe("ActionComponent_ApplyMood");
    expect("moodTemplate" in value.items[0].fields).toBe(true);
  });

  it("ActionComponent_Teleport resolves with teleportMode enum and stunDuration intRange", () => {
    const variant: ObjectTypeSchema = {
      fieldOrder: [
        "Class", "componentClass",
        ...Object.keys(COMPONENT_BASE_FIELDS).filter((k) => k !== "Class" && k !== "componentClass"),
        "teleportMode", "requiresLineOfSight", "teleportRange", "randomRange", "arrivalAlert", "arrivalAlertRadius",
        "stunDuration", "maxBodySize",
      ],
      fields: {
        ...COMPONENT_BASE_FIELDS,
        teleportMode: makeField({
          type: { kind: "enum" },
          defaultValue: "Selected",
          validationHints: { allowedValues: ["Caster", "RandomInRange", "Selected"] },
        }),
        requiresLineOfSight: makeField({ type: { kind: "boolean" }, defaultValue: false }),
        teleportRange: makeField({ type: { kind: "float" }, defaultValue: 0 }),
        randomRange: makeField({ type: { kind: "floatRange" }, defaultValue: "0~0" }),
        arrivalAlert: makeField({ type: { kind: "defReference" }, reference: { defType: "AlertDef", allowAbstract: false, scope: "allSources" } }),
        arrivalAlertRadius: makeField({ type: { kind: "integer" }, defaultValue: 0 }),
        stunDuration: makeField({ type: { kind: "intRange" }, defaultValue: "0~0" }),
        maxBodySize: makeField({ type: { kind: "float" }, defaultValue: 3.5 }),
      },
    };
    const catalog = makeCompCatalog("ActionComponent_Teleport", variant);
    const item = makeListItem("ActionComponent_Teleport", [
      { name: "teleportMode", value: "Selected" },
      { name: "stunDuration", value: "0~60" },
    ]);
    const compsDesc = getComponentsDescriptor(catalog, [item]);
    const value = compsDesc!.value as { kind: "objectList"; items: { schemaRef: string | null; fields: Record<string, unknown> }[] };
    expect(value.items[0].schemaRef).toBe("ActionComponent_Teleport");
    expect("teleportMode" in value.items[0].fields).toBe(true);
    expect("stunDuration" in value.items[0].fields).toBe(true);
  });

  it("ActionComponent_Projectile resolves with projectileTemplate field", () => {
    const variant = makeVariantSchema({
      projectileTemplate: makeField({
        type: { kind: "defReference" },
        reference: { defType: "EntityDef", allowAbstract: false, scope: "allSources" },
      }),
    });
    const catalog = makeCompCatalog("ActionComponent_Projectile", variant);
    const item = makeListItem("ActionComponent_Projectile", [
      { name: "projectileTemplate", value: "Bullet_Psychic" },
    ]);
    const compsDesc = getComponentsDescriptor(catalog, [item]);
    const value = compsDesc!.value as { kind: "objectList"; items: { schemaRef: string | null; fields: Record<string, unknown> }[] };
    expect(value.items[0].schemaRef).toBe("ActionComponent_Projectile");
    expect("projectileTemplate" in value.items[0].fields).toBe(true);
  });

  it("ActionComponent_Spray resolves with projectileTemplate and sprayCount", () => {
    const variant = makeVariantSchema({
      projectileTemplate: makeField({
        type: { kind: "defReference" },
        reference: { defType: "EntityDef", allowAbstract: false, scope: "allSources" },
      }),
      sprayCount: makeField({ type: { kind: "integer" }, defaultValue: 0 }),
      sprayEffect: makeField({
        type: { kind: "defReference" },
        reference: { defType: "EffectDef", allowAbstract: false, scope: "allSources" },
      }),
    });
    const catalog = makeCompCatalog("ActionComponent_Spray", variant);
    const item = makeListItem("ActionComponent_Spray", [
      { name: "projectileTemplate", value: "FireSpewProjectile" },
      { name: "sprayCount", value: "8" },
    ]);
    const compsDesc = getComponentsDescriptor(catalog, [item]);
    const value = compsDesc!.value as { kind: "objectList"; items: { schemaRef: string | null; fields: Record<string, unknown> }[] };
    expect(value.items[0].schemaRef).toBe("ActionComponent_Spray");
    expect("projectileTemplate" in value.items[0].fields).toBe(true);
    expect("sprayCount" in value.items[0].fields).toBe(true);
  });

  it("ActionComponent_AttachEffect resolves with visualEffect field", () => {
    const variant = makeVariantSchema({
      visualEffect: makeField({
        type: { kind: "defReference" },
        reference: { defType: "EffectDef", allowAbstract: false, scope: "allSources" },
      }),
      effectDuration: makeField({ type: { kind: "integer" }, defaultValue: -1 }),
      effectScale: makeField({ type: { kind: "float" }, defaultValue: 1 }),
    });
    const catalog = makeCompCatalog("ActionComponent_AttachEffect", variant);
    const item = makeListItem("ActionComponent_AttachEffect", [
      { name: "visualEffect", value: "Skip_Entry" },
    ]);
    const compsDesc = getComponentsDescriptor(catalog, [item]);
    const value = compsDesc!.value as { kind: "objectList"; items: { schemaRef: string | null; fields: Record<string, unknown> }[] };
    expect(value.items[0].schemaRef).toBe("ActionComponent_AttachEffect");
    expect("visualEffect" in value.items[0].fields).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 3. Fallback behavior for unknown component class
// ---------------------------------------------------------------------------

describe("TestActionDef component fallback for unknown Class", () => {
  it("does not crash for unknown component Class and falls back to base schema fields", () => {
    const catalog = makeCatalog();
    const schema = catalog.defTypes["TestActionDef"]!;
    const item = makeListItem("ActionComponent_FutureMadeUp", [
      { name: "someField", value: "someValue" },
    ]);
    const def = makeDef("TestActionDef", [makeListItemChild("components", [item])]);
    expect(() => buildFormDescriptors(def, schema, catalog)).not.toThrow();
    const descs = buildFormDescriptors(def, schema, catalog);
    const compsDesc = descs.find((x) => x.key === "components");
    expect(compsDesc).toBeDefined();
    expect(compsDesc!.control).toBe("objectList");
    const value = compsDesc!.value as { kind: "objectList"; items: { className: string; schemaRef: string | null; fields: Record<string, unknown> }[] };
    expect(value.kind).toBe("objectList");
    expect(value.items[0].className).toBe("ActionComponent_FutureMadeUp");
    // allowUnknown = true → resolveObjectSchema returns base schema so inherited fields remain visible.
    expect(value.items[0].schemaRef).toBe("ActionComponent");
  });
});

// ---------------------------------------------------------------------------
// 4. ActionVerb reuse across multiple def types
// ---------------------------------------------------------------------------

describe("ActionVerb reuse across TestActionDef and TestEventDef", () => {
  it("actionVerb under TestActionDef produces same nested fields as TestEventDef.eventVerb usage", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      defTypes: {
        TestActionDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["actionVerb"],
          fields: {
            actionVerb: makeField({
              type: { kind: "object", schemaRef: "ActionVerb" },
              xml: "object",
            }),
          },
        },
        TestEventDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["eventVerb"],
          fields: {
            eventVerb: makeField({
              type: { kind: "object", schemaRef: "ActionVerb" },
              xml: "object",
            }),
          },
        },
      },
      objectTypes: {
        ActionVerb: makeActionVerbSchema(),
      },
    };

    const actionSchema = catalog.defTypes["TestActionDef"]!;
    const eventSchema = catalog.defTypes["TestEventDef"]!;
    const actionDef = makeDef("TestActionDef", [makeChild("actionVerb", null, "object")]);
    const eventDef = { ...makeDef("TestEventDef", [makeChild("eventVerb", null, "object")]), defType: "TestEventDef" };

    const actionDescs = buildFormDescriptors(actionDef, actionSchema, catalog);
    const eventDescs = buildFormDescriptors(eventDef, eventSchema, catalog);

    const actionWarmup = actionDescs.find((d) => d.key === "actionVerb.warmupTime");
    const eventWarmup = eventDescs.find((d) => d.key === "eventVerb.warmupTime");

    expect(actionWarmup).toBeDefined();
    expect(eventWarmup).toBeDefined();
    expect(actionWarmup!.control).toBe(eventWarmup!.control);
    expect(actionWarmup!.readonly).toBe(eventWarmup!.readonly);
    expect(actionWarmup!.fieldPath).toEqual(["actionVerb", "warmupTime"]);
    expect(eventWarmup!.fieldPath).toEqual(["eventVerb", "warmupTime"]);
  });
});
