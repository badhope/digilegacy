<template>
  <div class="profile fade-in">
    <h2 style="font-size: 22px; font-weight: 700; margin-bottom: 24px;">{{ t('nav.profile') }}</h2>
    <p class="text-muted mb-32" style="font-size: 14px;">{{ t('profile.subtitle') }}</p>

    <div v-if="hasPlaceholder" class="card mb-24" role="alert" style="border-color: var(--warning); background: var(--warning-light);">
      <strong style="font-size: 14px;">{{ t('profile.placeholderTitle') }}</strong>
      <p style="font-size: 13px; margin-top: 6px;">{{ t('profile.placeholderDesc', { fields: placeholderFields.join('、') }) }}</p>
    </div>

    <div class="grid grid-2">
      <div class="card">
        <div class="card-title mb-16">{{ t('profile.basicTitle') }}</div>
        <div class="form-group">
          <label class="form-label" for="pf-full-name">{{ t('profile.fullName') }}</label>
          <input class="form-input" id="pf-full-name" v-model="profile.full_name" :placeholder="t('profile.fullNamePlaceholder')" />
        </div>
        <div class="form-group">
          <label class="form-label" for="pf-phone">{{ t('profile.phone') }}</label>
          <input class="form-input" id="pf-phone" v-model="profile.phone" :placeholder="t('profile.phonePlaceholder')" />
        </div>
        <div class="form-group">
          <label class="form-label" for="pf-email">{{ t('profile.email') }}</label>
          <input class="form-input" id="pf-email" v-model="profile.email" :placeholder="t('profile.emailPlaceholder')" />
        </div>
      </div>

      <div class="card">
        <div class="card-title mb-16">{{ t('profile.emergencyTitle') }}</div>
        <div class="form-group">
          <label class="form-label" for="pf-ec-name">{{ t('profile.fullName') }}</label>
          <input class="form-input" id="pf-ec-name" v-model="profile.emergency_contact_name" :placeholder="t('profile.emergencyNamePlaceholder')" />
        </div>
        <div class="form-group">
          <label class="form-label" for="pf-ec-phone">{{ t('profile.phone') }}</label>
          <input class="form-input" id="pf-ec-phone" v-model="profile.emergency_contact_phone" :placeholder="t('profile.contactPhonePlaceholder')" />
        </div>
        <div class="form-group">
          <label class="form-label" for="pf-ec-email">{{ t('profile.email') }}</label>
          <input class="form-input" id="pf-ec-email" v-model="profile.emergency_contact_email" :placeholder="t('profile.emergencyEmailPlaceholder')" />
        </div>
        <div class="form-group">
          <label class="form-label" for="pf-ec-relation">{{ t('profile.relation') }}</label>
          <input class="form-input" id="pf-ec-relation" v-model="profile.emergency_contact_relation" :placeholder="t('profile.relationPlaceholder')" />
        </div>
      </div>

      <div class="card">
        <div class="card-title mb-8">{{ t('heirs.title') }}</div>
        <p class="form-hint" style="margin-bottom: 12px;">{{ t('heirs.desc') }}</p>
        <div v-for="(h, idx) in heirs" :key="idx" class="heir-row">
          <div class="grid grid-2" style="gap: 8px;">
            <div class="form-group" style="margin: 0;">
              <label class="form-label" :for="'heir-name-' + idx">{{ t('profile.fullName') }}</label>
              <input class="form-input" :id="'heir-name-' + idx" v-model="h.name" />
            </div>
            <div class="form-group" style="margin: 0;">
              <label class="form-label" :for="'heir-rel-' + idx">{{ t('profile.relation') }}</label>
              <input class="form-input" :id="'heir-rel-' + idx" v-model="h.relation" :placeholder="t('profile.relationPlaceholder2')" />
            </div>
            <div class="form-group" style="margin: 0;">
              <label class="form-label" :for="'heir-phone-' + idx">{{ t('profile.phone') }}</label>
              <input class="form-input" :id="'heir-phone-' + idx" v-model="h.phone" />
            </div>
            <div class="form-group" style="margin: 0;">
              <label class="form-label" :for="'heir-email-' + idx">{{ t('profile.email') }}</label>
              <input class="form-input" :id="'heir-email-' + idx" v-model="h.email" />
            </div>
          </div>
          <div class="form-group" style="margin: 8px 0 0;">
            <label class="form-label" :for="'heir-share-' + idx">{{ t('heirs.shareNote') }}</label>
            <input class="form-input" :id="'heir-share-' + idx" v-model="h.share_note" :placeholder="t('heirs.shareNotePlaceholder')" />
          </div>
          <div class="flex items-center gap-12" style="margin-top: 8px; flex-wrap: wrap;">
            <label class="flex items-center gap-8" style="cursor: pointer; font-size: 13px;">
              <input type="radio" :name="'executor'" :value="idx" :checked="h.is_executor" @change="setExecutor(idx)" />
              {{ t('heirs.executor') }}
            </label>
            <button class="btn btn-danger btn-sm ml-auto" @click="removeHeir(idx)">{{ t('common.delete') }}</button>
          </div>
        </div>
        <p v-if="heirs.length === 0" class="form-hint">{{ t('heirs.empty') }}</p>
        <div style="display: flex; gap: 8px; margin-top: 12px; flex-wrap: wrap;">
          <button class="btn btn-secondary btn-sm" :disabled="heirs.length >= 20" @click="addHeir">{{ t('heirs.add') }}</button>
          <button class="btn btn-primary btn-sm" :disabled="heirsBusy" @click="saveHeirs">{{ heirsBusy ? t('common.saving') : t('heirs.save') }}</button>
        </div>
        <p v-if="heirsMsg" style="font-size: 12px; margin-top: 8px;" :style="{ color: heirsOk ? 'var(--success)' : 'var(--danger)' }" role="alert" aria-live="polite">{{ heirsMsg }}</p>
      </div>

      <div class="card">
        <div class="card-title mb-16">Dead Man's Switch</div>
        <p style="font-size: 13px; color: var(--gray-600);">
          {{ t('profile.dmsNote') }}
        </p>
        <p class="form-hint" style="margin-top: 8px;">
          {{ t('profile.dmsCurrent', { summary: dmsSummary }) }}
        </p>
        <router-link to="/dms" class="btn btn-secondary btn-sm mt-16">{{ t('profile.dmsGo') }} →</router-link>
      </div>
    </div>

    <div class="card mt-24">
      <div class="card-title mb-16">{{ t('profile.securityTitle') }}</div>
      <template v-if="security.hasMasterPassword">
        <p style="font-size: 13px; color: var(--gray-600);">{{ t('profile.securityEnabledDesc') }}</p>
        <button class="btn btn-secondary btn-sm" style="margin-top: 12px;" @click="lockNow"><LockClosedOutline /> {{ t('profile.lockNow') }}</button>

        <div style="margin-top: 20px; padding-top: 16px; border-top: 1px solid var(--gray-200, #e8e4de);">
          <div style="font-weight: 600; font-size: 14px; margin-bottom: 8px;">{{ t('profile.recoveryKey') }}</div>
          <p class="form-hint" style="margin-top: 0;">{{ t('profile.recoveryKeyHint') }}</p>
          <div style="font-size: 13px; color: var(--gray-600);">{{ t('profile.recoveryStatus') }}：{{ recoveryExists === null ? t('profile.recoveryChecking') : recoveryExists ? t('profile.recoveryGenerated') : t('profile.recoveryMissing') }}</div>
        </div>

        <div style="margin-top: 20px; padding-top: 16px; border-top: 1px solid var(--gray-200, #e8e4de);">
          <div style="font-weight: 600; font-size: 14px; margin-bottom: 12px;">{{ t('profile.changePassword') }}</div>
          <div class="grid grid-2" style="gap: 16px;">
            <div class="form-group">
              <label class="form-label" for="pf-old-pw">{{ t('profile.oldPassword') }}</label>
              <input class="form-input" id="pf-old-pw" type="password" v-model="oldPassword" :placeholder="t('profile.oldPassword')" />
            </div>
            <div class="form-group">
              <label class="form-label" for="pf-new-pw">{{ t('recovery.newPasswordLabel') }}</label>
              <input class="form-input" id="pf-new-pw" type="password" v-model="newPassword" :placeholder="t('profile.newPasswordPlaceholder')" />
            </div>
            <div class="form-group">
              <label class="form-label" for="pf-new-pw2">{{ t('recovery.confirmPasswordLabel') }}</label>
              <input class="form-input" id="pf-new-pw2" type="password" v-model="newPassword2" :placeholder="t('profile.confirmPasswordPlaceholder')" />
            </div>
          </div>
          <button class="btn btn-secondary btn-sm" :disabled="security.busy" @click="changeMasterPassword">{{ t('profile.changePasswordBtn') }}</button>
          <p v-if="changeMessage" role="alert" style="font-size: 13px; margin-top: 8px;" :style="{ color: changeOk ? 'var(--success)' : 'var(--danger)' }">{{ changeMessage }}</p>
        </div>
      </template>
      <template v-else>
        <p style="font-size: 13px; color: var(--gray-600);">{{ t('profile.securityDisabledDesc') }}</p>
        <div class="grid grid-2" style="margin-top: 12px; gap: 16px;">
          <div class="form-group">
            <label class="form-label" for="pf-master-pw">{{ t('profile.setPasswordLabel') }}</label>
            <input class="form-input" id="pf-master-pw" type="password" v-model="masterPassword" :placeholder="t('profile.setPasswordPlaceholder')" />
          </div>
          <div class="form-group">
            <label class="form-label" for="pf-master-pw2">{{ t('recovery.confirmPasswordLabel') }}</label>
            <input class="form-input" id="pf-master-pw2" type="password" v-model="masterPassword2" :placeholder="t('profile.confirmPlaceholder')" />
          </div>
        </div>
        <p class="form-hint">{{ t('profile.passwordWarning') }}</p>
        <button class="btn btn-primary btn-sm" style="margin-top: 12px;" :disabled="security.busy" @click="enableEncryption">{{ t('profile.enableEncryption') }}</button>
        <p v-if="securityMessage" role="alert" style="font-size: 13px; margin-top: 8px;" :style="{ color: securityOk ? 'var(--success)' : 'var(--danger)' }">{{ securityMessage }}</p>
      </template>
    </div>

    <!-- 整库加密（SQLCipher）：独立于主密码体系的第二层。桌面端 only。 -->
    <div class="card mt-24" v-if="isTauriEnv()" data-testid="db-encryption-card">
      <div class="card-title mb-8">{{ t('profile.dbEncTitle') }}</div>
      <p class="text-muted" style="font-size: 13px;">{{ t('profile.dbEncDesc') }}</p>
      <p class="form-hint">{{ t('profile.dbEncBoundary') }}</p>
      <div style="margin-top: 12px; display: flex; align-items: center; gap: 12px;">
        <span style="font-weight: 600; font-size: 14px;" :style="{ color: dbEncEnabled ? 'var(--success-strong, green)' : 'var(--gray-600)' }">
          {{ dbEncEnabled ? t('profile.dbEncOn') : t('profile.dbEncOff') }}
        </span>
        <button v-if="!dbEncEnabled" class="btn btn-primary btn-sm" style="margin-left: auto;" :disabled="dbEncBusy" data-testid="db-enc-enable" @click="enableDbEncryption">
          {{ t('profile.dbEncEnable') }}
        </button>
        <button v-else class="btn btn-secondary btn-sm" style="margin-left: auto;" :disabled="dbEncBusy" data-testid="db-enc-disable" @click="disableDbEncryption">
          {{ t('profile.dbEncDisable') }}
        </button>
      </div>
      <p v-if="dbEncMessage" role="status" style="font-size: 13px; margin-top: 8px;">{{ dbEncMessage }}</p>
    </div>

    <div class="card mt-24" v-if="recoveryCode" style="border-color: var(--warning); border-width: 2px;">
      <div class="card-title mb-16">{{ t('profile.recoveryOnce') }}</div>
      <p style="font-size: 13px; color: var(--gray-600);">
        {{ t('profile.recoveryOnceHint1') }}<strong>{{ t('profile.recoveryOnceHint2') }}</strong>{{ t('profile.recoveryOnceHint3') }}
      </p>
      <div class="recovery-code-box">{{ recoveryCode }}</div>
      <div style="display: flex; gap: 12px; margin-top: 12px; align-items: center; flex-wrap: wrap;">
        <button class="btn btn-secondary btn-sm" @click="copyRecoveryCode">{{ t('common.copy') }}</button>
        <label style="display: flex; align-items: center; gap: 8px; cursor: pointer; font-size: 13px;">
          <input type="checkbox" v-model="recoveryAck" style="width: 18px; height: 18px;" />
          {{ t('profile.recoveryWritten') }}
        </label>
        <button class="btn btn-primary btn-sm" :disabled="!recoveryAck" @click="recoveryCode = ''">{{ t('recovery.savedClose') }}</button>
      </div>
      <p class="form-hint">{{ t('profile.recoverySealedHint') }}</p>
    </div>

    <div class="card mt-24">
      <div class="card-title mb-16">{{ t('profile.backupTitle') }}</div>
      <p style="font-size: 13px; color: var(--gray-600);">{{ t('profile.backupDesc') }}</p>
      <div style="display: flex; gap: 12px; margin-top: 12px; flex-wrap: wrap;">
        <button class="btn btn-secondary btn-sm" @click="exportBackup">{{ t('profile.exportBackup') }}</button>
        <label class="sr-only" for="pf-backup-source-password">{{ t('profile.backupSourcePassword') }}</label>
        <input
          class="form-input"
          id="pf-backup-source-password"
          type="password"
          autocomplete="current-password"
          :placeholder="t('profile.backupSourcePasswordPlaceholder')"
          style="max-width: 260px;"
          v-model="backupSourcePassword"
        />
        <!-- 默认合并：覆盖是破坏性操作，必须走独立的、带警告的入口 -->
        <button class="btn btn-secondary btn-sm" @click="importBackup(false)">{{ t('profile.importBackup') }}</button>
        <button class="btn btn-danger btn-sm" @click="importBackup(true)">{{ t('profile.importReplace') }}</button>
      </div>
      <p v-if="backupMessage" role="alert" style="font-size: 13px; margin-top: 8px;" :style="{ color: backupOk ? 'var(--success)' : 'var(--danger)' }">{{ backupMessage }}</p>
      <p v-if="!security.hasMasterPassword" class="form-hint">{{ t('profile.backupPlainHint') }}</p>
      <p class="form-hint" style="margin-top: 10px;" aria-live="polite">{{ autoBackupSummary }}</p>
    </div>

    <!-- 检查点：备份是「按天」，检查点是「按你按下按钮的那一刻」。
         两者解决不同问题，不能互相替代。 -->
    <div class="card mt-24">
      <div class="card-header">
        <div>
          <div class="card-title">{{ t('profile.checkpointTitle') }}</div>
          <div class="card-subtitle">{{ t('profile.checkpointDesc') }}</div>
        </div>
        <button class="btn btn-primary btn-sm" :disabled="checkpointBusy" @click="createCheckpoint">
          {{ t('profile.checkpointCreate') }}
        </button>
      </div>

      <div v-if="checkpoints.length === 0" class="form-hint" style="padding: 8px 0;">
        {{ t('profile.checkpointEmpty') }}
      </div>

      <ul v-else class="checkpoint-list">
        <li v-for="cp in checkpoints" :key="cp.id" class="checkpoint-row">
          <div class="checkpoint-info">
            <strong>{{ cp.label || cp.id }}</strong>
            <span class="checkpoint-meta">
              {{ cp.created_at }} · {{ formatBytes(cp.size) }}
              <span v-if="cp.auto" class="tag tag-secondary" style="margin-left: 6px;">{{ t('profile.checkpointAuto') }}</span>
            </span>
          </div>
          <div class="checkpoint-actions">
            <button class="btn btn-secondary btn-sm" :disabled="checkpointBusy" @click="restoreCheckpoint(cp)">
              {{ t('profile.checkpointRestore') }}
            </button>
            <button class="btn btn-secondary btn-sm" :disabled="checkpointBusy" @click="removeCheckpoint(cp)">
              {{ t('common.delete') }}
            </button>
          </div>
        </li>
      </ul>

      <p v-if="checkpointMessage" role="alert" class="form-hint" style="margin-top: 10px;"
         :style="{ color: checkpointOk ? 'var(--success-strong)' : 'var(--danger-strong)' }">
        {{ checkpointMessage }}
      </p>
    </div>

    <div class="card mt-24">
      <div class="card-title mb-16">{{ t('profile.custodyTitle') }}</div>
      <p style="font-size: 13px; color: var(--gray-600);">{{ t('profile.custodyDesc') }}</p>
      <template v-if="!isTauri">
        <p class="form-hint">{{ t('profile.browserUnsupported') }}</p>
      </template>
      <template v-else>
        <div style="margin-top: 12px; display: grid; gap: 10px; max-width: 640px;">
          <div class="form-group" style="margin: 0;">
            <label class="form-label" for="custody-backup">{{ t('profile.custodyBackup') }}</label>
            <input class="form-input" id="custody-backup" v-model="custody.backup_location" :placeholder="t('profile.custodyBackupPlaceholder')" />
          </div>
          <div class="form-group" style="margin: 0;">
            <label class="form-label" for="custody-recovery">{{ t('profile.custodyRecovery') }}</label>
            <input class="form-input" id="custody-recovery" v-model="custody.recovery_code_location" :placeholder="t('profile.custodyRecoveryPlaceholder')" />
          </div>
        </div>
        <p class="form-hint">{{ t('profile.custodyHint') }}</p>
        <div style="margin-top: 12px;">
          <button class="btn btn-primary btn-sm" :disabled="custodyBusy" @click="saveCustody">{{ custodyBusy ? t('common.saving') : t('profile.custodySave') }}</button>
        </div>
        <p v-if="custodyMsg" style="font-size: 12px; margin-top: 8px;" :style="{ color: custodyOk ? 'var(--success)' : 'var(--danger)' }" role="alert" aria-live="polite">{{ custodyMsg }}</p>
      </template>
    </div>

    <!-- 继承人恢复包：单机应用唯一能让家人在持有者失能后解密数据的路径 -->
    <div class="card mt-24" id="recovery-kit-card">
      <div class="card-title mb-16">{{ t('kit.title') }}</div>
      <p style="font-size: 13px; color: var(--gray-600);">{{ t('kit.desc') }}</p>
      <template v-if="!isTauri">
        <p class="form-hint">{{ t('profile.browserUnsupported') }}</p>
      </template>
      <template v-else>
        <div v-if="kit.exists" class="form-hint" style="margin-top: 12px;" role="status">
          <strong>{{ t('kit.exists') }}</strong>
          {{ t('kit.existsDetail', { k: String(kit.threshold), n: String(kit.total), at: kit.created_at }) }}
          <span v-if="kit.stale" class="tag tag-warning" style="margin-left: 6px;">{{ t('kit.stale') }}</span>
        </div>
        <p v-else class="form-hint" style="margin-top: 12px;">{{ t('kit.none') }}</p>

        <div v-if="kit.stale && !kit.exists" class="form-hint">{{ t('kit.regenerateHint') }}</div>
        <p v-if="kitMessage" role="alert" style="font-size: 12px; margin-top: 8px;" :style="{ color: kitOk ? 'var(--success)' : 'var(--danger)' }">{{ kitMessage }}</p>

        <div style="margin-top: 12px; display: grid; gap: 10px; max-width: 640px;">
          <div class="flex gap-12" style="flex-wrap: wrap;">
            <div class="form-group" style="margin: 0; width: 140px;">
              <label class="form-label" for="kit-threshold">{{ t('kit.threshold') }}</label>
              <select class="form-select" id="kit-threshold" v-model.number="kitThreshold">
                <option v-for="n in 4" :key="n" :value="n + 1">{{ n + 1 }}</option>
              </select>
            </div>
            <div class="form-group" style="margin: 0; width: 140px;">
              <label class="form-label" for="kit-total">{{ t('kit.total') }}</label>
              <select class="form-select" id="kit-total" v-model.number="kitTotal">
                <option v-for="n in 9" :key="n + 1" :value="n + 1">{{ n + 1 }}</option>
              </select>
            </div>
          </div>
          <div class="form-group" style="margin: 0;">
            <label class="form-label" for="kit-pass">{{ t('kit.passphrase') }}</label>
            <input class="form-input" id="kit-pass" type="password" v-model="kitPassphrase" autocomplete="new-password" :placeholder="t('kit.passphrasePlaceholder')" />
            <p class="form-hint">{{ t('kit.passphraseHint') }}</p>
          </div>
        </div>
        <div style="margin-top: 12px; display: flex; gap: 12px; flex-wrap: wrap;">
          <button class="btn btn-primary btn-sm" :disabled="kitBusy || !security.hasMasterPassword || !security.unlocked" @click="generateKit">
            {{ kitBusy ? t('kit.generating') : t('kit.generate') }}
          </button>
          <button v-if="kit.exists" class="btn btn-danger btn-sm" :disabled="kitBusy" @click="clearKit">{{ t('kit.clear') }}</button>
        </div>
        <p class="form-hint">{{ t('kit.hint') }}</p>

        <!-- 恢复演练：生成过 ≠ 可用。这里用只读校验证明份额真的能恢复。 -->
        <div v-if="kit.exists" style="margin-top: 16px; padding-top: 16px; border-top: 1px solid var(--gray-200);">
          <div style="font-weight: 600; font-size: 13px; margin-bottom: 6px;">{{ t('kit.drill') }}</div>
          <p class="form-hint">{{ t('kit.drillDesc') }}</p>
          <div style="display: grid; gap: 8px; max-width: 640px; margin-top: 10px;">
            <div class="form-group" style="margin: 0;">
              <label class="form-label" for="kit-drill-passphrase">{{ t('kit.drillPassphrase') }}</label>
              <input
                class="form-input"
                id="kit-drill-passphrase"
                type="password"
                v-model="drillPassphrase"
                :placeholder="t('kit.drillPassphrasePlaceholder')"
                autocomplete="current-password"
              />
              <p class="form-hint">{{ t('kit.drillPassphraseHint') }}</p>
            </div>
            <textarea
              v-for="(_, i) in drillInputs"
              :key="i"
              class="form-input"
              rows="2"
              v-model="drillInputs[i]"
              :placeholder="t('kit.lockPlaceholder')"
              style="font-family: ui-monospace, Consolas, monospace; font-size: 12px;"
            ></textarea>
          </div>
          <div style="display: flex; gap: 12px; margin-top: 10px; flex-wrap: wrap;">
            <button class="btn btn-secondary btn-sm" @click="drillInputs.push('')">{{ t('kit.addShare') }}</button>
            <button class="btn btn-primary btn-sm" :disabled="drillBusy" @click="runDrill">
              {{ drillBusy ? t('kit.drillRunning') : t('kit.drillRun') }}
            </button>
          </div>
          <div v-if="drillResult" role="status" style="margin-top: 10px; font-size: 13px; line-height: 1.8;">
            <div>{{ t('kit.drillSelfTest') }}：
              <strong :style="{ color: drillResult.self_test_ok ? 'var(--success)' : 'var(--danger)' }">
                {{ drillResult.self_test_ok ? t('kit.drillSelfTestOk', { ms: String(drillResult.self_test_ms) }) : t('kit.drillSelfTestFail') }}
              </strong>
            </div>
            <div>{{ t('kit.drillValid', { valid: String(drillResult.valid), provided: String(drillResult.provided), threshold: String(drillResult.threshold) }) }}</div>
            <div>{{ drillResult.enough ? t('kit.drillEnough') : t('kit.drillNotEnough') }}</div>
            <div v-if="drillResult.passphrase_verified === true">{{ t('kit.drillPassphraseVerified') }}</div>
            <div v-else-if="drillResult.passphrase_verified === false">{{ t('kit.drillPassphraseFailed') }}</div>
            <div v-else-if="drillResult.errors.some(e => e.includes('只能验证结构'))">{{ t('kit.drillPassphraseHint') }}</div>
            <div>{{ drillResult.same_batch ? t('kit.drillSameBatch') : t('kit.drillMixedBatch') }}</div>
            <div v-if="drillResult.errors.length > 0">
              {{ t('kit.drillErrors') }}
              <ul style="margin: 4px 0 0 18px;">
                <li v-for="(e, i) in drillResult.errors" :key="i">{{ e }}</li>
              </ul>
            </div>
          </div>
        </div>
      </template>
    </div>

    <div class="card mt-24">
      <div class="card-title mb-16">{{ t('ai.title') }}</div>
      <p style="font-size: 13px; color: var(--gray-600);">{{ t('ai.desc') }}</p>
      <p class="form-hint" style="margin-top: 8px;">{{ t('ai.noService') }}</p>
      <template v-if="!isTauri">
        <p class="form-hint">{{ t('profile.browserUnsupported') }}</p>
      </template>
      <template v-else>
        <label class="flex items-center gap-8" style="cursor: pointer; margin-top: 12px;">
          <input type="checkbox" id="ai-enabled" v-model="assistant.enabled" style="width: 18px; height: 18px;" />
          <span style="font-weight: 500; font-size: 14px;">{{ t('ai.enable') }}</span>
        </label>
        <div style="margin-top: 10px; display: grid; gap: 10px; max-width: 640px;">
          <div class="form-group" style="margin: 0;">
            <label class="form-label" for="ai-endpoint">{{ t('ai.endpoint') }}</label>
            <input class="form-input" id="ai-endpoint" v-model="assistant.endpoint" placeholder="https://api.example.com/v1" autocomplete="off" />
            <p class="form-hint">{{ t('ai.endpointHint') }}</p>
          </div>
          <div class="form-group" style="margin: 0;">
            <label class="form-label" for="ai-model">{{ t('ai.model') }}</label>
            <input class="form-input" id="ai-model" v-model="assistant.model" placeholder="gpt-4o-mini / deepseek-chat / qwen-plus" autocomplete="off" />
          </div>
          <div class="form-group" style="margin: 0;">
            <label class="form-label" for="ai-key">{{ t('ai.key') }}</label>
            <input class="form-input" id="ai-key" type="password" v-model="assistantKey" :placeholder="assistant.has_key ? t('profile.tokenSaved') : t('ai.keyPlaceholder')" autocomplete="new-password" />
            <p class="form-hint">{{ t('ai.keyHint') }}</p>
          </div>
        </div>
        <div style="margin-top: 12px;">
          <button class="btn btn-primary btn-sm" :disabled="assistantBusy" @click="saveAssistant">{{ assistantBusy ? t('common.saving') : t('ai.save') }}</button>
        </div>
        <p v-if="assistantMsg" style="font-size: 12px; margin-top: 8px;" :style="{ color: assistantOk ? 'var(--success)' : 'var(--danger)' }" role="alert" aria-live="polite">{{ assistantMsg }}</p>
      </template>
    </div>

    <div class="card mt-24">
      <div class="card-title mb-16">{{ t('profile.smtpTitle') }}</div>
      <p style="font-size: 13px; color: var(--gray-600);">{{ t('profile.smtpDesc') }}</p>
      <template v-if="!isTauri">
        <p class="form-hint">{{ t('profile.browserUnsupported') }}</p>
      </template>
      <template v-else>
        <div style="margin-top: 12px; display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 10px; max-width: 720px;">
          <div class="form-group" style="margin: 0;">
            <label class="form-label" for="smtp-host">{{ t('profile.smtpHost') }}</label>
            <input class="form-input" id="smtp-host" v-model="smtp.host" placeholder="smtp.example.com" autocomplete="off" />
          </div>
          <div class="form-group" style="margin: 0;">
            <label class="form-label" for="smtp-port">{{ t('profile.smtpPort') }}</label>
            <input class="form-input" id="smtp-port" type="number" min="1" max="65535" v-model.number="smtp.port" />
          </div>
          <div class="form-group" style="margin: 0;">
            <label class="form-label" for="smtp-from">{{ t('profile.smtpFrom') }}</label>
            <input class="form-input" id="smtp-from" v-model="smtp.from_addr" placeholder="noreply@example.com" autocomplete="off" />
          </div>
          <div class="form-group" style="margin: 0;">
            <label class="form-label" for="smtp-user">{{ t('profile.smtpUser') }}</label>
            <input class="form-input" id="smtp-user" v-model="smtp.username" autocomplete="off" />
          </div>
          <div class="form-group" style="margin: 0;">
            <label class="form-label" for="smtp-pass">{{ t('profile.smtpPass') }}</label>
            <input class="form-input" id="smtp-pass" type="password" v-model="smtpPassword" :placeholder="smtp.has_password ? t('profile.tokenSaved') : t('profile.smtpPassPlaceholder')" autocomplete="new-password" />
          </div>
          <div class="form-group" style="margin: 0;">
            <label class="form-label" for="smtp-test-to">{{ t('profile.smtpTestTo') }}</label>
            <input class="form-input" id="smtp-test-to" v-model="smtpTestTo" :placeholder="profile.email || t('profile.smtpTestToPlaceholder')" autocomplete="off" />
          </div>
        </div>
        <label class="flex items-center gap-8" style="cursor: pointer; margin-top: 8px;">
          <input type="checkbox" v-model="smtp.allow_insecure" style="width: 18px; height: 18px;" />
          <span style="font-size: 13px;">{{ t('profile.smtpAllowInsecure') }}</span>
        </label>
        <p class="form-hint">{{ t('profile.smtpHint') }}</p>
        <div style="display: flex; gap: 12px; flex-wrap: wrap; margin-top: 12px;">
          <button class="btn btn-primary btn-sm" :disabled="smtpBusy" @click="saveSmtp">{{ smtpBusy ? t('common.saving') : t('profile.smtpSave') }}</button>
          <button class="btn btn-secondary btn-sm" :disabled="smtpBusy" @click="sendTestMail">{{ t('profile.smtpTest') }}</button>
        </div>
        <p v-if="smtpMsg" style="font-size: 12px; margin-top: 8px;" :style="{ color: smtpOk ? 'var(--success)' : 'var(--danger)' }" role="alert" aria-live="polite">{{ smtpMsg }}</p>
      </template>
    </div>

    <div class="card mt-24">
      <div class="card-title mb-16">{{ t('profile.heartbeatTitle') }}</div>
      <p style="font-size: 13px; color: var(--gray-600);">{{ t('profile.heartbeatDesc1') }}<strong>{{ t('profile.heartbeatDesc2') }}</strong>{{ t('profile.heartbeatDesc3') }}</p>
      <template v-if="!isTauri">
        <p class="form-hint">{{ t('profile.browserUnsupported') }}</p>
      </template>
      <template v-else>
        <div style="margin-top: 12px; display: grid; gap: 10px; max-width: 520px;">
          <label class="flex items-center gap-8" style="cursor: pointer;">
            <input type="checkbox" v-model="heartbeatEnabled" style="width: 18px; height: 18px;" />
            <span style="font-weight: 500; font-size: 14px;">{{ t('profile.heartbeatEnable') }}</span>
          </label>
          <div class="form-group" style="margin: 0;">
            <label class="form-label" for="hb-endpoint">{{ t('profile.heartbeatEndpoint') }}</label>
            <input class="form-input" id="hb-endpoint" v-model="heartbeatEndpoint" :placeholder="t('profile.heartbeatEndpointPlaceholder')" />
            <p class="form-hint">{{ t('profile.heartbeatSelfHost') }}</p>
          </div>
          <div class="form-group" style="margin: 0;">
            <label class="form-label" for="hb-token">{{ t('profile.heartbeatToken') }}</label>
            <input class="form-input" id="hb-token" type="password" v-model="heartbeatToken" :placeholder="heartbeatHasToken ? t('profile.tokenSaved') : t('profile.tokenPlaceholder')" />
          </div>
          <div>
            <button class="btn btn-primary btn-sm" :disabled="heartbeatBusy" @click="saveHeartbeatConfig">{{ heartbeatBusy ? t('common.saving') : t('profile.saveHeartbeat') }}</button>
          </div>
        </div>
        <p v-if="heartbeatMsg" style="font-size: 12px; color: var(--gray-600); margin-top: 8px;" aria-live="polite">{{ heartbeatMsg }}</p>
        <div style="display: flex; align-items: center; gap: 12px; margin-top: 12px; flex-wrap: wrap;">
          <button class="btn btn-secondary btn-sm" @click="sendHeartbeat">{{ t('profile.testHeartbeat') }}</button>
          <span v-if="heartbeatTestMsg" style="font-size: 12px; color: var(--gray-600);" aria-live="polite">{{ heartbeatTestMsg }}</span>
        </div>
        <div style="margin-top: 16px; padding-top: 12px; border-top: 1px solid var(--gray-200, #e8e4de);">
          <label class="flex items-center gap-8" style="cursor: pointer;">
            <input type="checkbox" v-model="autostartEnabled" style="width: 18px; height: 18px;" @change="toggleAutostart" />
            <span style="font-weight: 500; font-size: 14px;">{{ t('profile.autostart') }}</span>
          </label>
          <p class="form-hint">{{ t('profile.autostartHint') }}</p>
        </div>
      </template>
    </div>

    <div class="card mt-24">
      <div class="card-title mb-16">{{ t('profile.patchTitle') }}</div>
      <p style="font-size: 13px; color: var(--gray-600);">{{ t('profile.patchDesc') }}</p>
      <template v-if="!isTauri">
        <p class="form-hint">{{ t('profile.browserUnsupported') }}</p>
      </template>
      <template v-else>
        <p class="form-hint" style="margin-top: 8px;">{{ t('profile.patchCount') }} <strong>{{ patchCount }}</strong> {{ t('profile.patchCountSuffix') }}。</p>
        <div style="display: flex; gap: 10px; flex-wrap: wrap; margin-top: 12px;">
          <button class="btn btn-secondary btn-sm" @click="importPatch">{{ t('profile.importPatch') }}</button>
          <button class="btn btn-secondary btn-sm" @click="exportPatch">{{ patchCount > 0 ? t('profile.exportPatch') : t('profile.exportPatchTemplate') }}</button>
          <button class="btn btn-secondary btn-sm" :disabled="patchCount === 0" @click="clearPatch">{{ t('profile.clearPatch') }}</button>
        </div>
        <p v-if="patchMsg" style="font-size: 12px; color: var(--gray-600); margin-top: 8px;" aria-live="polite">{{ patchMsg }}</p>
      </template>
    </div>

    <div class="card mt-24" style="text-align: center;">
      <button class="btn btn-primary btn-lg" @click="saveProfile">{{ t('profile.saveProfile') }}</button>
      <p class="text-muted mt-16" style="font-size: 12px;">
        {{ security.hasMasterPassword ? t('profile.saveHintEncrypted') : t('profile.saveHintPlain') }}
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';

import { useI18n } from '../i18n';
import { useDashboardStore } from '../stores/dashboard';
import LockClosedOutline from '@vicons/ionicons5/es/LockClosedOutline';
import { useSecurityStore } from '../stores/security';
import { usePlatformsStore } from '../stores/platforms';
import { useAssetsStore } from '../stores/assets';
import { toast, confirmDialog } from '../composables/feedback';
import { invoke, isTauriEnv } from '../api/invoke';
import type { ApiResponse, AssistantConfig, CustodyHints, Heir, UserProfile } from '../types';

const { t } = useI18n();
const dashboard = useDashboardStore();
const security = useSecurityStore();
const platforms = usePlatformsStore();
const assets = useAssetsStore();

const masterPassword = ref('');
const masterPassword2 = ref('');
const securityMessage = ref('');

// ── 整库加密（SQLCipher）：状态与开关 ──
const dbEncEnabled = ref(false);
const dbEncBusy = ref(false);
const dbEncMessage = ref('');

async function loadDbEncryptionStatus() {
  try {
    const res = await invoke<ApiResponse<{ enabled: boolean }>>('get_db_encryption_status');
    if (res.success && res.data) dbEncEnabled.value = res.data.enabled;
  } catch {
    /* 浏览器预览/旧后端无此命令：保持默认关闭显示 */
  }
}

async function enableDbEncryption() {
  dbEncBusy.value = true;
  try {
    const res = await invoke<ApiResponse<void>>('enable_db_encryption');
    dbEncMessage.value = res.message;
    if (res.success) dbEncEnabled.value = true;
  } catch (e) {
    dbEncMessage.value = String(e);
  } finally {
    dbEncBusy.value = false;
  }
}

async function disableDbEncryption() {
  dbEncBusy.value = true;
  try {
    const res = await invoke<ApiResponse<void>>('disable_db_encryption');
    dbEncMessage.value = res.message;
    if (res.success) dbEncEnabled.value = false;
  } catch (e) {
    dbEncMessage.value = String(e);
  } finally {
    dbEncBusy.value = false;
  }
}
const securityOk = ref(false);

const oldPassword = ref('');
const newPassword = ref('');
const newPassword2 = ref('');
const changeMessage = ref('');
const changeOk = ref(false);

const backupMessage = ref('');

interface CheckpointMeta {
  id: string;
  label: string;
  created_at: string;
  size: number;
  auto: boolean;
}
const checkpoints = ref<CheckpointMeta[]>([]);
const checkpointBusy = ref(false);
const checkpointMessage = ref('');
const checkpointOk = ref(false);

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

async function loadCheckpoints() {
  try {
    const res = await invoke<ApiResponse<CheckpointMeta[]>>('list_checkpoints');
    if (res.success && res.data) checkpoints.value = res.data;
  } catch (e) {
    console.error('读取检查点失败:', e);
  }
}

async function createCheckpoint() {
  checkpointBusy.value = true;
  checkpointMessage.value = '';
  try {
    const res = await invoke<ApiResponse<CheckpointMeta>>('create_checkpoint', {
      label: t('profile.checkpointDefaultLabel'),
    });
    checkpointOk.value = res.success;
    checkpointMessage.value = res.message;
    if (res.success) await loadCheckpoints();
  } catch (e) {
    checkpointOk.value = false;
    checkpointMessage.value = String(e);
  } finally {
    checkpointBusy.value = false;
  }
}

async function restoreCheckpoint(cp: CheckpointMeta) {
  // 回滚是破坏性操作：必须显式确认，并说明会覆盖当前数据
  const okToRun = window.confirm(t('profile.checkpointRestoreConfirm', { label: cp.label || cp.id }));
  if (!okToRun) return;
  checkpointBusy.value = true;
  checkpointMessage.value = '';
  try {
    const res = await invoke<ApiResponse<string>>('restore_checkpoint', {
      id: cp.id,
      sourcePassword: backupSourcePassword.value,
    });
    checkpointOk.value = res.success;
    checkpointMessage.value = res.message;
    if (res.success) {
      await loadCheckpoints();
      await security.refresh();
    }
  } catch (e) {
    checkpointOk.value = false;
    checkpointMessage.value = String(e);
  } finally {
    checkpointBusy.value = false;
  }
}

async function removeCheckpoint(cp: CheckpointMeta) {
  checkpointBusy.value = true;
  try {
    await invoke<ApiResponse<null>>('delete_checkpoint', { id: cp.id });
    await loadCheckpoints();
  } catch (e) {
    checkpointOk.value = false;
    checkpointMessage.value = String(e);
  } finally {
    checkpointBusy.value = false;
  }
}
const backupOk = ref(false);
const backupSourcePassword = ref('');

const isTauri = isTauriEnv();
const recoveryExists = ref<boolean | null>(null);

/** SMTP 配置：密码只回显「已保存」，不回传明文。 */
interface SmtpForm {
  host: string;
  port: number;
  from_addr: string;
  username: string;
  has_password: boolean;
  allow_insecure: boolean;
}
const smtp = ref<SmtpForm>({
  host: '',
  port: 587,
  from_addr: '',
  username: '',
  has_password: false,
  allow_insecure: false,
});
const smtpPassword = ref('');
const smtpTestTo = ref('');
/** 继承人名单：整组保存，必须恰好一位执行人（后端也会校验）。 */
const heirs = ref<Heir[]>([]);
const heirsMsg = ref('');
const heirsOk = ref(false);
const heirsBusy = ref(false);

function addHeir() {
  heirs.value.push({
    name: '',
    relation: '',
    phone: '',
    email: '',
    share_note: '',
    is_executor: heirs.value.length === 0,
    sort_order: heirs.value.length,
  });
}

function removeHeir(idx: number) {
  const wasExecutor = heirs.value[idx]?.is_executor;
  heirs.value.splice(idx, 1);
  // 删掉执行人后自动把第一位设为执行人，避免保存时才报错
  if (wasExecutor && heirs.value.length > 0) {
    heirs.value.forEach((h, i) => { h.is_executor = i === 0; });
  }
}

function setExecutor(idx: number) {
  heirs.value.forEach((h, i) => { h.is_executor = i === idx; });
}

async function loadHeirs() {
  try {
    const res = await invoke<ApiResponse<Heir[]>>('get_heirs');
    if (res.success && res.data) heirs.value = res.data;
  } catch (e) {
    heirsMsg.value = String(e);
  }
}

async function saveHeirs() {
  heirsBusy.value = true;
  heirsMsg.value = '';
  try {
    const payload = heirs.value.map((h, i) => ({
      ...h,
      name: h.name.trim(),
      relation: h.relation.trim(),
      phone: h.phone.trim(),
      email: h.email.trim(),
      share_note: h.share_note.trim(),
      sort_order: i,
    }));
    const res = await invoke<ApiResponse<null>>('set_heirs', { heirs: payload });
    heirsOk.value = res.success;
    heirsMsg.value = res.message;
    if (res.success) await loadHeirs();
  } catch (e) {
    heirsOk.value = false;
    heirsMsg.value = String(e);
  } finally {
    heirsBusy.value = false;
  }
}

/** AI 助手：默认关闭、无内置端点，用户自填。Key 只回显「已保存」。 */
const assistant = ref<AssistantConfig>({ enabled: false, endpoint: '', model: '', has_key: false });
const assistantKey = ref('');
const assistantMsg = ref('');
const assistantOk = ref(false);
const assistantBusy = ref(false);

/** 保管位置：备份文件与恢复码放在哪。 */
const custody = ref<CustodyHints>({ backup_location: '', recovery_code_location: '' });
const custodyMsg = ref('');
const custodyOk = ref(false);
const custodyBusy = ref(false);

const smtpMsg = ref('');
const smtpOk = ref(false);
const smtpBusy = ref(false);

const heartbeatMsg = ref('');
const heartbeatTestMsg = ref('');
const heartbeatEnabled = ref(false);
const heartbeatEndpoint = ref('');
const heartbeatToken = ref('');
const heartbeatHasToken = ref(false);
const heartbeatBusy = ref(false);
const patchCount = ref(0);
const patchMsg = ref('');
const autostartEnabled = ref(false);
const autoBackupSummary = ref('');

/** 自动备份是静默功能：不显示状态时用户无法判断它是否真的在跑。 */
async function loadAutoBackupStatus() {
  try {
    const res = await invoke<ApiResponse<{ count: number; latest_name: string | null; latest_size: number | null }>>('get_backup_status');
    if (!res.success || !res.data) {
      autoBackupSummary.value = t('profile.autoBackupEmpty');
      return;
    }
    const { count, latest_name, latest_size } = res.data;
    autoBackupSummary.value = count > 0 && latest_name
      ? t('profile.autoBackupStatus', {
          count,
          name: latest_name,
          size: Math.max(1, Math.round((latest_size ?? 0) / 1024)),
        })
      : t('profile.autoBackupEmpty');
  } catch (e) {
    autoBackupSummary.value = t('profile.autoBackupFailed', { e: String(e) });
  }
}

const recoveryCode = ref('');
const recoveryAck = ref(false);

function copyRecoveryCode() {
  navigator.clipboard.writeText(recoveryCode.value).then(
    () => { toast(t('profile.copied')); },
    () => { toast(t('profile.copyFailed'), 'error'); },
  );
}

async function loadHeartbeatConfig() {
  try {
    const res = await invoke<ApiResponse<{ enabled: boolean; has_token: boolean; endpoint: string }>>('get_heartbeat_config');
    if (res.success && res.data) {
      heartbeatEnabled.value = res.data.enabled;
      heartbeatHasToken.value = res.data.has_token;
      heartbeatEndpoint.value = res.data.endpoint;
    }
  } catch {
    // 读取失败时保持默认空态，用户保存时会重新写入
  }
}

async function loadAssistant() {
  try {
    const res = await invoke<ApiResponse<AssistantConfig>>('get_assistant_config');
    if (res.success && res.data) assistant.value = res.data;
  } catch (e) {
    assistantMsg.value = String(e);
  }
}

async function saveAssistant() {
  assistantBusy.value = true;
  assistantMsg.value = '';
  try {
    const res = await invoke<ApiResponse<null>>('set_assistant_config', {
      enabled: assistant.value.enabled,
      endpoint: assistant.value.endpoint,
      model: assistant.value.model,
      // 空串表示「不改 Key」：后端用 None 保留原密文
      apiKey: assistantKey.value ? assistantKey.value : null,
    });
    assistantOk.value = res.success;
    assistantMsg.value = res.message;
    if (res.success) {
      assistantKey.value = '';
      await loadAssistant();
    }
  } catch (e) {
    assistantOk.value = false;
    assistantMsg.value = String(e);
  } finally {
    assistantBusy.value = false;
  }
}

async function loadCustody() {
  try {
    const res = await invoke<ApiResponse<CustodyHints>>('get_custody_hints');
    if (res.success && res.data) custody.value = res.data;
  } catch (e) {
    custodyMsg.value = String(e);
  }
}

// ── 继承人恢复包 ──
interface KitStatus {
  exists: boolean;
  threshold?: number;
  total?: number;
  created_at?: string;
  passphrase_protected?: boolean;
  stale?: boolean;
}
const kit = ref<KitStatus>({ exists: false });
const kitBusy = ref(false);
const kitMessage = ref('');
const kitOk = ref(false);
const kitThreshold = ref(3);
const kitTotal = ref(5);
const kitPassphrase = ref('');

async function loadKit() {
  if (!isTauri) return;
  try {
    const res = await invoke<ApiResponse<KitStatus>>('heir_recovery_status');
    if (res.success && res.data) {
      kit.value = res.data;
      if (res.data.exists && res.data.threshold) kitThreshold.value = res.data.threshold;
      if (res.data.exists && res.data.total) kitTotal.value = res.data.total;
    }
  } catch (e) {
    kitMessage.value = String(e);
  }
}

async function generateKit() {
  kitBusy.value = true;
  kitMessage.value = '';
  if (kitThreshold.value > kitTotal.value) {
    kitMessage.value = t('kit.thresholdTooHigh');
    kitOk.value = false;
    kitBusy.value = false;
    return;
  }
  try {
    const res = await invoke<ApiResponse<{ path: string; threshold: number; total: number }>>(
      'generate_heir_recovery_kit',
      {
        threshold: kitThreshold.value,
        total: kitTotal.value,
        passphrase: kitPassphrase.value.trim() ? kitPassphrase.value : null,
      },
    );
    kitOk.value = res.success;
    kitMessage.value = res.success
      ? t('kit.generated', { path: res.data?.path ?? '', n: String(res.data?.total ?? kitTotal.value) })
      : res.message;
    if (res.success) {
      kitPassphrase.value = '';
      await loadKit();
    }
  } catch (e) {
    kitOk.value = false;
    kitMessage.value = String(e);
  } finally {
    kitBusy.value = false;
  }
}

// ── 恢复演练 ──
interface DrillResult {
  self_test_ok: boolean;
  self_test_ms: number;
  provided: number;
  valid: number;
  threshold: number;
  enough: boolean;
  same_batch: boolean;
  passphrase_verified?: boolean | null;
  errors: string[];
}
const drillInputs = ref<string[]>(['']);
const drillPassphrase = ref('');
const drillBusy = ref(false);
const drillResult = ref<DrillResult | null>(null);

async function runDrill() {
  drillBusy.value = true;
  drillResult.value = null;
  try {
    const shares = drillInputs.value.map(s => s.trim()).filter(Boolean);
    const passphrase = drillPassphrase.value.trim() || null;
    const res = await invoke<ApiResponse<DrillResult>>('run_recovery_drill', { shares, passphrase });
    if (res.success && res.data) {
      drillResult.value = res.data;
      drillPassphrase.value = '';
    } else {
      kitOk.value = false;
      kitMessage.value = res.message;
    }
  } catch (e) {
    kitOk.value = false;
    kitMessage.value = String(e);
  } finally {
    drillBusy.value = false;
  }
}

async function clearKit() {
  const ok = await confirmDialog({
    title: t('kit.clearTitle'),
    message: t('kit.clearMessage'),
    danger: true,
  });
  if (!ok) return;
  kitBusy.value = true;
  try {
    const res = await invoke<ApiResponse<null>>('clear_heir_recovery');
    kitOk.value = res.success;
    kitMessage.value = res.message;
    await loadKit();
  } catch (e) {
    kitOk.value = false;
    kitMessage.value = String(e);
  } finally {
    kitBusy.value = false;
  }
}

async function saveCustody() {
  custodyBusy.value = true;
  custodyMsg.value = '';
  try {
    const res = await invoke<ApiResponse<null>>('set_custody_hints', { hints: custody.value });
    custodyOk.value = res.success;
    custodyMsg.value = res.message;
  } catch (e) {
    custodyOk.value = false;
    custodyMsg.value = String(e);
  } finally {
    custodyBusy.value = false;
  }
}

async function loadSmtpConfig() {
  try {
    const res = await invoke<ApiResponse<SmtpForm>>('get_smtp_config');
    if (res.success && res.data) {
      smtp.value = { ...res.data, port: res.data.port || 587 };
    }
  } catch (e) {
    smtpMsg.value = String(e);
  }
}

async function saveSmtp() {
  smtpBusy.value = true;
  smtpMsg.value = '';
  try {
    const res = await invoke<ApiResponse<null>>('set_smtp_config', {
      host: smtp.value.host,
      port: smtp.value.port,
      fromAddr: smtp.value.from_addr,
      username: smtp.value.username,
      // 空串表示「不改密码」：后端用 None 保留原密文，避免误清空。
      password: smtpPassword.value ? smtpPassword.value : null,
      allowInsecure: smtp.value.allow_insecure,
    });
    smtpOk.value = res.success;
    smtpMsg.value = res.message;
    if (res.success) {
      smtpPassword.value = '';
      await loadSmtpConfig();
  await loadCheckpoints();
    }
  } catch (e) {
    smtpOk.value = false;
    smtpMsg.value = String(e);
  } finally {
    smtpBusy.value = false;
  }
}

async function sendTestMail() {
  smtpBusy.value = true;
  smtpMsg.value = '';
  try {
    const to = smtpTestTo.value.trim() || profile.value.email.trim();
    const res = await invoke<ApiResponse<string>>('send_test_email', { recipient: to });
    smtpOk.value = res.success;
    smtpMsg.value = res.success ? res.message : res.message;
  } catch (e) {
    smtpOk.value = false;
    smtpMsg.value = String(e);
  } finally {
    smtpBusy.value = false;
  }
}

async function saveHeartbeatConfig() {
  heartbeatBusy.value = true;
  heartbeatMsg.value = '';
  try {
    const res = await invoke<ApiResponse<null>>('set_heartbeat_config', {
      enabled: heartbeatEnabled.value,
      token: heartbeatToken.value,
      endpoint: heartbeatEndpoint.value,
    });
    heartbeatMsg.value = res.message;
    if (res.success) {
      heartbeatToken.value = '';
      await loadHeartbeatConfig();
    }
  } catch (e) {
    heartbeatMsg.value = String(e);
  } finally {
    heartbeatBusy.value = false;
  }
}

async function sendHeartbeat() {
  heartbeatTestMsg.value = t('profile.sending');
  try {
    const res = await invoke<ApiResponse<{ last_result: string }>>('send_heartbeat_now');
    if (res.success && res.data) {
      heartbeatTestMsg.value = res.data.last_result === 'ok'
        ? t('profile.heartbeatOk')
        : t('profile.heartbeatNotSent');
    } else {
      heartbeatTestMsg.value = res.message;
    }
  } catch (e) {
    heartbeatTestMsg.value = String(e);
  }
}

async function toggleAutostart() {
  try {
    const res = await invoke<ApiResponse<null>>('set_autostart', { enabled: autostartEnabled.value });
    if (!res.success) {
      autostartEnabled.value = !autostartEnabled.value;
      toast(res.message, 'error');
    }
  } catch (e) {
    autostartEnabled.value = !autostartEnabled.value;
    toast(String(e), 'error');
  }
}

async function changeMasterPassword() {
  changeMessage.value = '';
  if (!oldPassword.value || newPassword.value.length < 8) {
    changeMessage.value = t('profile.changeReq');
    changeOk.value = false;
    return;
  }
  if (newPassword.value !== newPassword2.value) {
    changeMessage.value = t('profile.passwordMismatch');
    changeOk.value = false;
    return;
  }
  try {
    const res = await invoke<ApiResponse<{ status: { has_master_password: boolean; unlocked: boolean }; recovery_code: string }>>('change_master_password', {
      oldPassword: oldPassword.value,
      newPassword: newPassword.value,
      confirmPassword: newPassword2.value,
    });
    changeOk.value = res.success;
    changeMessage.value = res.message;
    if (res.success) {
      oldPassword.value = '';
      newPassword.value = '';
      newPassword2.value = '';
      await security.refresh();
      // 改密即轮换恢复密钥：复用"仅显示这一次"卡片强制用户抄写新码
      if (res.data?.recovery_code) {
        recoveryCode.value = res.data.recovery_code;
        recoveryAck.value = false;
      }
    }
  } catch (e) {
    changeOk.value = false;
    changeMessage.value = String(e);
  }
}

async function exportBackup() {
  backupMessage.value = '';
  try {
    const res = await invoke<ApiResponse<string>>('export_backup');
    backupOk.value = res.success;
    backupMessage.value = res.success ? t('profile.backupExported', { path: res.data }) : res.message;
  } catch (e) {
    backupOk.value = false;
    backupMessage.value = String(e);
  }
}

async function refreshImportedState() {
  await Promise.all([
    dashboard.loadUser(),
    dashboard.loadStats(),
    dashboard.loadCheckInStatus(),
    security.refresh(),
    assets.loadAssets(),
    platforms.refreshPlatforms(),
    loadHeirs(),
    loadCustody(),
    loadAssistant(),
    loadSmtpConfig(),
    loadHeartbeatConfig(),
    loadKit(),
    loadPatchStatus(),
    loadAutoBackupStatus(),
  ]);
  if (dashboard.user) {
    profile.value = { ...profile.value, ...dashboard.user };
    if (dashboard.checkInStatus) {
      profile.value.dms_enabled = dashboard.checkInStatus.dms_enabled;
      profile.value.check_in_interval_days = dashboard.checkInStatus.interval_days;
    }
  }
}

async function importBackup(replace = false) {
  backupMessage.value = '';
  if (!isTauri) {
    backupOk.value = false;
    backupSourcePassword.value = '';
    backupMessage.value = t('profile.webImportUnsupported');
    return;
  }
  const ok = await confirmDialog({
    title: replace ? t('profile.importReplaceTitle') : t('profile.importBackupTitle'),
    message: replace ? t('profile.importReplaceMessage') : t('profile.importBackupMessage'),
    confirmText: replace ? t('profile.importReplaceConfirm') : t('profile.importBackupConfirm'),
    danger: replace,
  });
  if (!ok) {
    backupSourcePassword.value = '';
    return;
  }
  try {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const { readTextFile } = await import('@tauri-apps/plugin-fs');
    const selected = await open({ multiple: false, filters: [{ name: t('profile.backupFilterName'), extensions: ['json'] }] });
    if (!selected || typeof selected !== 'string') return;
    const content = await readTextFile(selected);
    const sourcePassword = backupSourcePassword.value || null;
    const res = await invoke<ApiResponse<string>>('import_backup', {
      content,
      mode: replace ? 'replace' : 'merge',
      sourcePassword,
    });
    backupOk.value = res.success;
    backupMessage.value = res.success ? `${res.message}：${res.data}` : res.message;
    backupSourcePassword.value = '';
    if (res.success) {
      // 备份已写入数据库：必须让所有数据视图重新拉取，
      // 否则用户看到的是导入前的旧列表，会误判「恢复失败」并反复导入。
      await refreshImportedState();
    }
  } catch (e) {
    backupOk.value = false;
    backupSourcePassword.value = '';
    backupMessage.value = t('profile.importFailed', { e: String(e) });
  }
}

async function importPatch() {
  patchMsg.value = '';
  try {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const { readTextFile } = await import('@tauri-apps/plugin-fs');
    const selected = await open({ multiple: false, filters: [{ name: t('profile.patchFilterName'), extensions: ['json'] }] });
    if (!selected || typeof selected !== 'string') return;
    const content = await readTextFile(selected);
    const res = await invoke<ApiResponse<string>>('import_platform_patch', { content });
    patchMsg.value = res.success ? `${res.message}：${res.data}` : res.message;
    if (res.success) {
      await loadPatchStatus();
      await platforms.refreshPlatforms();
    }
  } catch (e) {
    patchMsg.value = t('profile.importFailed', { e: String(e) });
  }
}

async function exportPatch() {
  patchMsg.value = '';
  try {
    // 后端直接写进应用数据目录（与备份导出同路径策略），无需前端写文件权限
    const res = await invoke<ApiResponse<string>>('export_platform_patch');
    patchMsg.value = res.success ? `${res.message}：${res.data}` : res.message;
  } catch (e) {
    patchMsg.value = t('profile.exportFailed', { e: String(e) });
  }
}

async function clearPatch() {
  const ok = await confirmDialog({
    title: t('profile.clearPatchTitle'),
    message: t('profile.clearPatchMessage'),
    confirmText: t('common.delete'),
    danger: true,
  });
  if (!ok) return;
  try {
    const res = await invoke<ApiResponse<number>>('clear_platform_patch');
    patchMsg.value = res.success ? t('profile.patchCleared', { count: res.data }) : res.message;
    if (res.success) {
      await loadPatchStatus();
      await platforms.refreshPlatforms();
    }
  } catch (e) {
    patchMsg.value = t('profile.clearFailed', { e: String(e) });
  }
}

async function loadPatchStatus() {
  try {
    const res = await invoke<ApiResponse<number>>('platform_patch_status');
    if (res.success) patchCount.value = Number(res.data ?? 0);
  } catch {
    patchCount.value = 0;
  }
}

async function enableEncryption() {
  securityMessage.value = '';
  if (masterPassword.value.length < 8) {
    securityMessage.value = t('profile.passwordTooShort');
    securityOk.value = false;
    return;
  }
  if (masterPassword.value !== masterPassword2.value) {
    securityMessage.value = t('profile.passwordMismatch');
    securityOk.value = false;
    return;
  }
  const result = await security.setMasterPassword(masterPassword.value);
  if (!result.ok) {
    securityMessage.value = result.message;
    securityOk.value = false;
    return;
  }
  securityMessage.value = t('profile.encryptionEnabled');
  securityOk.value = true;
  masterPassword.value = '';
  masterPassword2.value = '';
  // 恢复密钥强制展示，直到用户确认抄写完成
  recoveryCode.value = result.recoveryCode;
  recoveryAck.value = false;
}

async function lockNow() {
  await security.lockNow();
}

const profile = ref<UserProfile>({
  full_name: '', phone: '', email: '', id_last4: '',
  emergency_contact_name: '', emergency_contact_phone: '', emergency_contact_relation: '', emergency_contact_email: '',
  heir_primary_name: '', heir_primary_phone: '', heir_primary_relation: '', heir_primary_email: '',
  check_in_interval_days: 30, last_check_in: null, dms_enabled: false,
  created_at: '', updated_at: '',
});

/** DMS 配置的唯一事实源是 dms_config（安心传承页维护），档案页只读展示。 */
const dmsSummary = computed(() => {
  const st = dashboard.checkInStatus;
  if (!st) return t('profile.dmsReading');
  if (!st.dms_enabled) return t('profile.dmsDisabled');
  return t('profile.dmsSummary', { days: st.interval_days });
});

onMounted(async () => {
  void loadDbEncryptionStatus();
  // 安全状态由本页自己拉取一次：不能假设 App 的初始化一定先完成
  // （单测/深链直接进入本页时 App 尚未 ready，会显示「未启用加密」的错误状态）。
  await Promise.all([security.refresh(), dashboard.loadUser(), dashboard.loadCheckInStatus()]);
  if (dashboard.user) {
    profile.value = { ...profile.value, ...dashboard.user };
    // 档案表里的 DMS 副本可能是旧值；显示与保存都跟随 dms_config
    if (dashboard.checkInStatus) {
      profile.value.dms_enabled = dashboard.checkInStatus.dms_enabled;
      profile.value.check_in_interval_days = dashboard.checkInStatus.interval_days;
    }
  }
  // 四块配置在浏览器演示模式下同样有 mock 实现，且界面会渲染它们；
  // 只在桌面版加载会让演示页显示「尚未登记继承人」等空态。
  await loadCustody();
  await loadKit();
  await loadAssistant();
  await loadHeirs();
  await loadSmtpConfig();
  // 检查点在浏览器预览里也有 mock 实现，因此不能塞进 if (isTauri)：
  // 否则演示页永远显示「还没有检查点」，用户看不到这个能力。
  await loadCheckpoints();

  if (isTauri) {
    try {
      const res = await invoke<ApiResponse<boolean>>('get_recovery_seal_exists');
      recoveryExists.value = res.success ? Boolean(res.data) : null;
    } catch {
      recoveryExists.value = null;
    }
    await loadHeartbeatConfig();
    await loadPatchStatus();
    await loadAutoBackupStatus();
    try {
      const { isEnabled } = await import('@tauri-apps/plugin-autostart');
      autostartEnabled.value = await isEnabled();
    } catch {
      autostartEnabled.value = false;
    }
  }

});

const PROFILE_PLACEHOLDER = '【加密数据 · 需解锁】';

/** 哪些档案字段是「未解密占位符」——这类值不能保存回后端。 */
const placeholderFields = computed(() => {
  const p = profile.value;
  const hits: string[] = [];
  const pairs: Array<[string, string]> = [
    ['姓名', p.full_name], ['手机号', p.phone], ['邮箱', p.email],
    ['紧急联系人姓名', p.emergency_contact_name], ['紧急联系人电话', p.emergency_contact_phone],
    ['紧急联系人邮箱', p.emergency_contact_email],
    ['继承人姓名', p.heir_primary_name], ['继承人电话', p.heir_primary_phone],
    ['继承人邮箱', p.heir_primary_email],
  ];
  for (const [label, value] of pairs) {
    if (value === PROFILE_PLACEHOLDER) hits.push(label);
  }
  return hits;
});

const hasPlaceholder = computed(() => placeholderFields.value.length > 0);

async function saveProfile() {
  if (hasPlaceholder.value) {
    toast(t('profile.placeholderBlocked', { fields: placeholderFields.value.join('、') }), 'error');
    return;
  }
  try {
    await dashboard.saveUser(profile.value);
    // 不再把档案页里的旧 DMS 值回写 dms_config：那会让用户在「安心传承」
    // 设好的 90 天周期，被一次无关的「保存档案」静默改回 30 天。
    toast(t('profile.saveOk'));
  } catch (e) {
    // 保存失败必须给用户明确反馈，不得静默
    toast(t('profile.saveFailed', { e: String(e) }), 'error');
  }
}
</script>

<style scoped>
.checkpoint-list {
  list-style: none;
  padding: 0;
  margin: 8px 0 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.checkpoint-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 14px;
  background: var(--gray-50);
  border-radius: 10px;
  flex-wrap: wrap;
}
.checkpoint-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.checkpoint-info strong {
  font-size: 14px;
  color: var(--gray-900);
  overflow-wrap: break-word;
}
.checkpoint-meta {
  font-size: 12px;
  color: var(--gray-600);
  font-variant-numeric: tabular-nums;
}
.checkpoint-actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}
[data-theme='dark'] .checkpoint-row { background: var(--gray-800); }
[data-theme='dark'] .checkpoint-info strong { color: var(--gray-100); }

.recovery-code-box {
  margin-top: 12px;
  padding: 12px 14px;
  border: 2px dashed var(--warning, #b7791f);
  border-radius: 8px;
  background: rgba(183, 121, 31, 0.08);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 15px;
  letter-spacing: 1px;
  word-break: break-all;
  text-align: center;
}
</style>
