import React from "react";
import { Piece as CheckersPiece } from "./checkers";
import { fieldWidth, fieldHeight } from "./Field";
import Crown from "./Crown";
import {
    lightPieceColor,
    darkPieceColor,
    selectedOutlineColor,
} from "./colors";
import "./animations.css";

export interface PieceProps {
    onClick(): void;
    piece: CheckersPiece;
    index: number;
    selected: boolean;
    flipCrown?: boolean;
}

export default function Piece(props: PieceProps) {
    const radius = fieldWidth / 3;
    const row = Math.floor(props.index / 4);
    const column =
        row % 2 == 0 ? (props.index % 4) * 2 + 1 : (props.index % 4) * 2;
    const fill =
        props.piece.team === "Light" ? lightPieceColor : darkPieceColor;
    const stroke = props.selected ? selectedOutlineColor : "";

    return (
        <>
            <circle
                className="animate-position"
                cx={column * fieldWidth + fieldWidth / 2}
                cy={row * fieldHeight + fieldHeight / 2}
                r={radius}
                fill={fill}
                stroke={stroke}
                strokeWidth="5"
                onClick={props.onClick}
            />
            {props.piece.kind === "King" && (
                <Crown
                    row={row}
                    column={column}
                    flip={props.flipCrown}
                    onClick={props.onClick}
                />
            )}
        </>
    );
}
