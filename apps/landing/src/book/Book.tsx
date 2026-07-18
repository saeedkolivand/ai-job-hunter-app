"use client";

// The notebook assembly around the one live page (PageMesh renders separately at
// z = 0 as the top page). Book builds the static shell that gives the page a body
// to sit in:
//   - a back cover board (kraft board, thicker) behind the stack
//   - a page-stack block just behind the top page (paper-edge thickness)
//   - the spiral binding along the top edge: ONE InstancedMesh of a torus ring,
//     EVERY instance matrix set at init (an unset instance would render white)
//   - the front cover, hinged about the top binding edge -- page 0's exit. The
//     composer (the single frame writer) rotates coverRef about x from
//     channels[0].exitP; this component only exposes the pivot group + geometry.
//
// The cover board + page-stack edge use the baked-paper shell materials
// (bookMaterials): the board reuses the shared kraft bake at low frequency, the
// stack is a procedural page-layers edge. The spiral binding stays a
// MeshStandardMaterial (metal), lit by the one scene directional light added in
// RipbookExperience. Geometry/material for the imperatively-built binding + the
// two shell materials are disposed on unmount; the JSX primitives rely on R3F's
// default auto-dispose.

import { type RefObject, useEffect, useLayoutEffect, useMemo, useRef } from "react";
import {
  type Group,
  type InstancedMesh,
  type Material,
  Matrix4,
  MeshStandardMaterial,
  TorusGeometry,
} from "three";

import { createBoardMaterial, createStackEdgeMaterial } from "@/book/bookMaterials";
import { PAGE_H, PAGE_W } from "@/engine/pages";

// Kraft board overhangs the page slightly and is a few mm thick; the stack is a
// thin block of "pages"; the front cover closes just in front of the top page.
const BOARD_OVERHANG = 0.12;
const BOARD_THICK = 0.03;
const COVER_Z = 0.06;
const BACK_Z = -0.13;
const STACK_Z = -0.05;
const STACK_THICK = 0.08;

// Spiral binding stays a standard metal material (kept per the M2 contract).
const BINDING_METAL = "#9a9ea6";

const BINDING_COUNT = 18;
const BINDING_MARGIN_X = 0.16;

// Spiral binding: one InstancedMesh of a small torus ring, laid along the top
// edge. The torus is rotated so its hole axis runs along x (the binding rod). We
// set EVERY instance matrix -- a never-set instance would render at the identity
// (a white ring stacked at the origin), the classic InstancedMesh gotcha.
function SpiralBinding() {
  const ref = useRef<InstancedMesh>(null);

  const geo = useMemo(() => {
    const g = new TorusGeometry(0.055, 0.016, 8, 16);
    g.rotateY(Math.PI / 2);
    return g;
  }, []);
  const mat = useMemo(
    () =>
      new MeshStandardMaterial({
        color: BINDING_METAL,
        metalness: 0.85,
        roughness: 0.4,
      }),
    [],
  );

  useLayoutEffect(() => {
    const mesh = ref.current;
    if (!mesh) return;
    const m = new Matrix4();
    const usableW = PAGE_W - BINDING_MARGIN_X * 2;
    for (let k = 0; k < BINDING_COUNT; k += 1) {
      const f = k / (BINDING_COUNT - 1);
      const x = -usableW / 2 + f * usableW;
      m.makeTranslation(x, PAGE_H / 2 + 0.02, 0);
      mesh.setMatrixAt(k, m);
    }
    mesh.instanceMatrix.needsUpdate = true;
  }, []);

  useEffect(
    () => () => {
      geo.dispose();
      mat.dispose();
    },
    [geo, mat],
  );

  return <instancedMesh ref={ref} args={[geo, mat, BINDING_COUNT]} />;
}

// Front cover, hinged about the top binding edge. The pivot group sits at the top
// edge; the board is offset down so it covers the page when closed. rotation.x is
// written each frame by the composer (channels[0].exitP), so nothing here reads
// scroll state.
function FrontCover({
  coverRef,
  boardMat,
}: {
  coverRef: RefObject<Group | null>;
  boardMat: Material;
}) {
  return (
    <group ref={coverRef} position={[0, PAGE_H / 2, COVER_Z]}>
      <mesh position={[0, -PAGE_H / 2, 0]} material={boardMat}>
        <boxGeometry
          args={[PAGE_W + BOARD_OVERHANG, PAGE_H + BOARD_OVERHANG, BOARD_THICK]}
        />
      </mesh>
    </group>
  );
}

export default function Book({ coverRef }: { coverRef: RefObject<Group | null> }) {
  // The board (front + back covers) and the page-stack edge are custom baked-
  // shell materials. Built once, disposed on unmount.
  const boardMat = useMemo(() => createBoardMaterial(), []);
  const stackMat = useMemo(() => createStackEdgeMaterial(), []);
  useEffect(
    () => () => {
      boardMat.dispose();
      stackMat.dispose();
    },
    [boardMat, stackMat],
  );

  return (
    <group>
      {/* Back cover board. */}
      <mesh position={[0, 0, BACK_Z]} material={boardMat}>
        <boxGeometry
          args={[PAGE_W + BOARD_OVERHANG, PAGE_H + BOARD_OVERHANG, BOARD_THICK]}
        />
      </mesh>

      {/* Page stack behind the top page (paper-edge thickness). */}
      <mesh position={[0, 0, STACK_Z]} material={stackMat}>
        <boxGeometry args={[PAGE_W - 0.02, PAGE_H - 0.02, STACK_THICK]} />
      </mesh>

      <SpiralBinding />
      <FrontCover coverRef={coverRef} boardMat={boardMat} />
    </group>
  );
}
