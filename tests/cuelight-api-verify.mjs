/**
 * CueLight API 集成验证脚本
 *
 * 用法: node tests/cuelight-api-verify.mjs
 *
 * 环境变量（可选覆盖默认值）:
 *   CUELIGHT_SERVER=https://cuelight.app
 *   CUELIGHT_TOKEN=cue_xsYA70c-o1xoCChggbzQsppQ47Jwq_MwdEeRHuP6RS8
 *   CUELIGHT_PROJECT_ID=9b8be474-b09f-4b72-a667-04bc27ea6623
 */

const SERVER = process.env.CUELIGHT_SERVER || "https://cuelight.app";
const TOKEN = process.env.CUELIGHT_TOKEN || "cue_xsYA70c-o1xoCChggbzQsppQ47Jwq_MwdEeRHuP6RS8";
const PROJECT_ID = process.env.CUELIGHT_PROJECT_ID || "9b8be474-b09f-4b72-a667-04bc27ea6623";

let passed = 0;
let failed = 0;
const results = [];

async function apiGet(path, query = {}) {
  const url = new URL(path, SERVER);
  for (const [k, v] of Object.entries(query)) {
    if (v) url.searchParams.set(k, v);
  }
  const res = await fetch(url.toString(), {
    headers: { Authorization: `Bearer ${TOKEN}` },
  });
  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw new Error(`HTTP ${res.status}: ${text.slice(0, 200)}`);
  }
  return res.json();
}

async function apiPost(path, body) {
  const url = new URL(path, SERVER);
  const res = await fetch(url.toString(), {
    method: "POST",
    headers: {
      Authorization: `Bearer ${TOKEN}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw new Error(`HTTP ${res.status}: ${text.slice(0, 200)}`);
  }
  return res.json();
}

function assert(name, condition, detail = "") {
  if (condition) {
    passed++;
    results.push({ name, status: "PASS", detail });
    console.log(`  ✅ ${name}`);
  } else {
    failed++;
    results.push({ name, status: "FAIL", detail });
    console.log(`  ❌ ${name} — ${detail}`);
  }
}

// ───────────────────────────────────────────────────────────
// 1. Health Check
// ───────────────────────────────────────────────────────────
async function testHealth() {
  console.log("\n📡 1. 健康检查 GET /api/health");
  try {
    const data = await apiGet("/api/health");
    assert("返回 status ok", data.status === "ok", JSON.stringify(data));
    assert("包含 timestamp", typeof data.timestamp === "number");
    return true;
  } catch (e) {
    assert("健康检查成功", false, e.message);
    return false;
  }
}

// ───────────────────────────────────────────────────────────
// 2. Project Detail
// ───────────────────────────────────────────────────────────
async function testProjectDetail() {
  console.log("\n📋 2. 项目详情 GET /api/projects/:id");
  try {
    const data = await apiGet(`/api/projects/${PROJECT_ID}`);
    console.log("    项目字段:", Object.keys(data).join(", "));
    assert("返回对象", typeof data === "object" && data !== null);
    assert("包含 projectType", typeof data.projectType === "string", `projectType=${data.projectType}`);
    assert("包含 videoAspectRatio", typeof data.videoAspectRatio === "string", `ratio=${data.videoAspectRatio}`);
    assert("包含 episodes 数组", Array.isArray(data.episodes), `episodes=${data.episodes?.length}`);
    if (data.title) {
      assert("包含 title", true, `title=${data.title}`);
    }
    return data;
  } catch (e) {
    assert("项目详情成功", false, e.message);
    return null;
  }
}

// ───────────────────────────────────────────────────────────
// 3. Bible (World View + Style)
// ───────────────────────────────────────────────────────────
async function testBible() {
  console.log("\n📖 3. 圣经 GET /api/projects/:id/bible");
  try {
    const data = await apiGet(`/api/projects/${PROJECT_ID}/bible`);
    console.log("    Bible 字段:", Object.keys(data).join(", "));
    assert("返回对象", typeof data === "object" && data !== null);
    if (data.worldView) {
      assert("worldView 非空字符串", typeof data.worldView === "string" && data.worldView.length > 0, `len=${data.worldView.length}`);
    }
    if (data.stylePrompt) {
      assert("stylePrompt 非空字符串", typeof data.stylePrompt === "string" && data.stylePrompt.length > 0, `len=${data.stylePrompt.length}`);
    }
    return data;
  } catch (e) {
    assert("圣经接口成功", false, e.message);
    return null;
  }
}

// ───────────────────────────────────────────────────────────
// 4. Episodes
// ───────────────────────────────────────────────────────────
async function testEpisodes() {
  console.log("\n🎬 4. 集数列表 GET /api/projects/:id/episodes");
  try {
    const data = await apiGet(`/api/projects/${PROJECT_ID}/episodes`);
    console.log("    集数数量:", data.length);
    assert("返回数组", Array.isArray(data));
    assert("集数 > 0", data.length > 0, `count=${data.length}`);
    if (data.length > 0) {
      const ep = data[0];
      console.log("    首集字段:", Object.keys(ep).join(", "));
      assert("首集有 id", typeof ep.id === "string");
      if (ep.title) assert("首集有 title", typeof ep.title === "string", `title=${ep.title}`);
      if (ep.episodeNumber != null) assert("首集有 episodeNumber", true, `num=${ep.episodeNumber}`);
    }
    return data;
  } catch (e) {
    assert("集数列表成功", false, e.message);
    return [];
  }
}

// ───────────────────────────────────────────────────────────
// 5. Characters
// ───────────────────────────────────────────────────────────
async function testCharacters() {
  console.log("\n👤 5. 角色列表 GET /api/projects/:id/characters");
  try {
    const data = await apiGet(`/api/projects/${PROJECT_ID}/characters`);
    console.log("    角色数量:", data.length);
    assert("返回数组", Array.isArray(data));
    if (data.length > 0) {
      const ch = data[0];
      console.log("    首角色字段:", Object.keys(ch).join(", "));
      assert("角色有 id", typeof ch.id === "string");
      assert("角色有 name", typeof ch.name === "string", `name=${ch.name}`);
    }
    return data;
  } catch (e) {
    assert("角色列表成功", false, e.message);
    return [];
  }
}

// ───────────────────────────────────────────────────────────
// 6. Scenes
// ───────────────────────────────────────────────────────────
async function testScenes() {
  console.log("\n🏞️ 6. 场景列表 GET /api/projects/:id/scenes");
  try {
    const data = await apiGet(`/api/projects/${PROJECT_ID}/scenes`);
    console.log("    场景数量:", data.length);
    assert("返回数组", Array.isArray(data));
    if (data.length > 0) {
      const sc = data[0];
      console.log("    首场景字段:", Object.keys(sc).join(", "));
      assert("场景有 id", typeof sc.id === "string");
      assert("场景有 name", typeof sc.name === "string", `name=${sc.name}`);
    }
    return data;
  } catch (e) {
    assert("场景列表成功", false, e.message);
    return [];
  }
}

// ───────────────────────────────────────────────────────────
// 7. Storyboards (需要 episodeId)
// ───────────────────────────────────────────────────────────
async function testStoryboards(episodes) {
  if (episodes.length === 0) {
    console.log("\n📋 7. 分镜列表 — 跳过（无集数）");
    return;
  }
  const epId = episodes[0].id;
  console.log(`\n📋 7. 分镜列表 GET /api/episodes/${epId}/storyboards (首集)`);
  try {
    const data = await apiGet(`/api/episodes/${epId}/storyboards`);
    console.log("    分镜数量:", data.length);
    assert("返回数组", Array.isArray(data));
    if (data.length > 0) {
      const sb = data[0];
      console.log("    首分镜字段:", Object.keys(sb).join(", "));
      assert("分镜有 id", typeof sb.id === "string");
      if (sb.videoPrompt) assert("分镜有 videoPrompt", typeof sb.videoPrompt === "string", `prompt=${sb.videoPrompt.slice(0, 60)}...`);
      if (sb.sceneNumber != null) assert("分镜有 sceneNumber", true, `scene=${sb.sceneNumber}`);
      if (sb.referenceCharacterIds) assert("分镜有 referenceCharacterIds", Array.isArray(sb.referenceCharacterIds));
    }
    return data;
  } catch (e) {
    assert("分镜列表成功", false, e.message);
    return [];
  }
}

// ───────────────────────────────────────────────────────────
// 8. Video Assets
// ───────────────────────────────────────────────────────────
async function testVideoAssets() {
  console.log("\n🎥 8. 视频资产 GET /api/projects/:id/video-assets");
  try {
    const data = await apiGet(`/api/projects/${PROJECT_ID}/video-assets`);
    console.log("    视频资产数量:", Array.isArray(data) ? data.length : "非数组");
    assert("返回数组", Array.isArray(data));
    if (Array.isArray(data) && data.length > 0) {
      console.log("    首资产字段:", Object.keys(data[0]).join(", "));
    }
    return data;
  } catch (e) {
    assert("视频资产成功", false, e.message);
    return [];
  }
}

// ───────────────────────────────────────────────────────────
// 9. Models
// ───────────────────────────────────────────────────────────
async function testModels() {
  console.log("\n🤖 9. 模型列表 GET /v1/models");
  try {
    const data = await apiGet("/v1/models");
    const models = data.data || data;
    console.log("    模型数量:", Array.isArray(models) ? models.length : "非数组");
    assert("返回数组", Array.isArray(models));
    if (Array.isArray(models) && models.length > 0) {
      console.log("    首模型:", JSON.stringify(models[0]).slice(0, 120));
    }
    return models;
  } catch (e) {
    assert("模型列表成功", false, e.message);
    return [];
  }
}

// ───────────────────────────────────────────────────────────
// 10. Project List (绑定选择)
// ───────────────────────────────────────────────────────────
async function testProjectList() {
  console.log("\n📂 10. 项目列表 GET /api/projects");
  try {
    const data = await apiGet("/api/projects", { projectType: "non_director" });
    const list = Array.isArray(data) ? data : data?.data ?? [];
    console.log("    项目数量:", list.length);
    assert("返回数组", Array.isArray(list));
    if (list.length > 0) {
      console.log("    首项目字段:", Object.keys(list[0]).join(", "));
      assert("首项目有 id", typeof list[0].id === "string");
    }
    return list;
  } catch (e) {
    assert("项目列表成功", false, e.message);
    return [];
  }
}

// ───────────────────────────────────────────────────────────
// Main
// ───────────────────────────────────────────────────────────
async function main() {
  console.log("╔══════════════════════════════════════════════════╗");
  console.log("║     CueLight API 集成验证测试                    ║");
  console.log("╠══════════════════════════════════════════════════╣");
  console.log(`║ Server:    ${SERVER.padEnd(37)}║`);
  console.log(`║ Project:   ${PROJECT_ID.padEnd(37)}║`);
  console.log("╚══════════════════════════════════════════════════╝");

  const healthy = await testHealth();
  if (!healthy) {
    console.log("\n❌ 健康检查失败，中止测试");
    process.exit(1);
  }

  const project = await testProjectDetail();
  await testBible();
  const episodes = await testEpisodes();
  await testCharacters();
  await testScenes();
  await testStoryboards(episodes);
  await testVideoAssets();
  await testModels();
  await testProjectList();

  console.log("\n══════════════════════════════════════════════════");
  console.log(`结果: ${passed} 通过, ${failed} 失败, 共 ${passed + failed} 项`);
  console.log("══════════════════════════════════════════════════");

  if (failed > 0) {
    console.log("\n失败项:");
    results.filter((r) => r.status === "FAIL").forEach((r) => {
      console.log(`  ❌ ${r.name}: ${r.detail}`);
    });
  }

  process.exit(failed > 0 ? 1 : 0);
}

main().catch((e) => {
  console.error("未捕获错误:", e);
  process.exit(1);
});
