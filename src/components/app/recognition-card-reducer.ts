import type {ColorProbeForm, RecognitionCardForm} from "@/components/app/recognition-types";

export type RecognitionCardAction =
    | {type: "patch"; cardId: string; patch: Partial<RecognitionCardForm>}
    | {type: "update"; cardId: string; update: (card: RecognitionCardForm) => RecognitionCardForm}
    | {type: "patchProbe"; cardId: string; probeIndex: number; patch: Partial<ColorProbeForm>}
    | {type: "remove"; cardId: string}
    | {type: "replace"; cards: RecognitionCardForm[]}
    | {type: "transform"; update: (cards: RecognitionCardForm[]) => RecognitionCardForm[]};

export function recognitionCardReducer(
    cards: RecognitionCardForm[],
    action: RecognitionCardAction,
): RecognitionCardForm[] {
    if (action.type === "replace") return action.cards;
    if (action.type === "transform") return action.update(cards);
    if (action.type === "remove") return cards.filter((card) => card.id !== action.cardId);
    return cards.map((card) => {
        if (card.id !== action.cardId) return card;
        if (action.type === "patch") return {...card, ...action.patch};
        if (action.type === "update") return action.update(card);
        return {
            ...card,
            colorProbes: card.colorProbes.map((probe, index) =>
                index === action.probeIndex ? {...probe, ...action.patch} : probe,
            ),
        };
    });
}
