/// <reference types="vitest/globals" />
import "@testing-library/jest-dom";

// Tauri APIをモック（テスト環境ではネイティブAPIが使えない）
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: vi.fn().mockResolvedValue(true),
}));
