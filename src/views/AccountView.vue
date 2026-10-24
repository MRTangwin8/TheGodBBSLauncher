<script setup lang="ts">
import UserCard from "../components/UserCard.vue";

// 勋章彩蛋：以后按游戏时长 / 行为解锁
const medals = [
  { icon: "⛏", name: "初来乍到", desc: "第一次启动游戏", unlocked: false },
  { icon: "🕐", name: "肝帝", desc: "累计游戏 100 小时", unlocked: false },
  { icon: "🧱", name: "考古学家", desc: "安装一个远古版本", unlocked: false },
  { icon: "🔧", name: "模组大师", desc: "安装任意模组加载器", unlocked: false },
  { icon: "👑", name: "神权", desc: "修改过 JVM 参数 —— 你掌控一切", unlocked: false },
];
</script>

<template>
  <div class="account">
    <div>
      <div class="box">
        <div class="box-title">账号</div>
        <div class="box-body">
          <div class="mod-tip">【版主提示】离线账号 / 微软正版 / 外置登录正在施工，下个版本见。</div>
        </div>
      </div>
      <div class="box">
        <div class="box-title">勋章墙</div>
        <div class="box-body medals">
          <div v-for="m in medals" :key="m.name" class="medal" :class="{ locked: !m.unlocked }" :title="m.desc">
            <div class="medal-icon">{{ m.icon }}</div>
            <div class="medal-name">{{ m.name }}</div>
            <div class="muted medal-desc">{{ m.desc }}</div>
          </div>
        </div>
      </div>
    </div>
    <UserCard />
  </div>
</template>

<style scoped>
.account { display: grid; grid-template-columns: 1fr 230px; gap: 12px; align-items: start; }
.medals { display: grid; grid-template-columns: repeat(auto-fill, minmax(120px, 1fr)); gap: 10px; }
.medal { text-align: center; padding: 10px 6px; border: 1px solid var(--border); border-radius: var(--radius); background: var(--bg-box-alt); }
.medal.locked { filter: grayscale(1); opacity: 0.55; }
.medal-icon { font-size: 26px; }
.medal-name { font-weight: bold; margin-top: 4px; }
.medal-desc { font-size: 12px; }
</style>
