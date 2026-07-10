pub(super) enum GraphicPreviewStrategy {
    Single,
    DirectionalMulti,
    FolderCollection,
    Appearances,
    SpecialWrapper,
    Unknown,
}

pub(super) fn strategy_for_graphic_class(graphic_class: &str) -> GraphicPreviewStrategy {
    match graphic_class {
        "Graphic_Single"
        | "Graphic_Single_AgeSecs"
        | "Graphic_Single_SquashNStretch"
        | "Graphic_Fleck"
        | "Graphic_FleckPulse"
        | "Graphic_FleckSplash"
        | "Graphic_Mote"
        | "Graphic_MoteWithAgeSecs"
        | "Graphic_MoteWithParentRotation"
        | "Graphic_Gas"
        | "Graphic_FadesInOut"
        | "Graphic_ActivityMask"
        | "Graphic_Terrain"
        | "Graphic_Tiling" => GraphicPreviewStrategy::Single,

        "Graphic_Multi"
        | "Graphic_Multi_AgeSecs"
        | "Graphic_Multi_BuildingWorking"
        | "Graphic_Multi_Mote" => GraphicPreviewStrategy::DirectionalMulti,

        "Graphic_Random"
        | "Graphic_Indexed"
        | "Graphic_StackCount"
        | "Graphic_Flicker"
        | "Graphic_Cluster"
        | "Graphic_ClusterTight"
        | "Graphic_ActivityStaged"
        | "Graphic_Genepack"
        | "Graphic_MealVariants"
        | "Graphic_MoteRandom"
        | "Graphic_ActivityMaskRandom"
        | "Graphic_Indexed_SquashNStretch"
        | "GraphicMote_RandomWithAgeSecs" => GraphicPreviewStrategy::FolderCollection,

        "Graphic_Appearances" => GraphicPreviewStrategy::Appearances,

        "Graphic_Linked"
        | "Graphic_LinkedAsymmetric"
        | "Graphic_LinkedCornerFiller"
        | "Graphic_LinkedCornerOverlay"
        | "Graphic_LinkedTransmitter"
        | "Graphic_LinkedTransmitterOverlay"
        | "Graphic_RandomRotated"
        | "Graphic_Shadow"
        | "Graphic_PawnBodySilhouette" => GraphicPreviewStrategy::SpecialWrapper,

        _ => GraphicPreviewStrategy::Unknown,
    }
}
