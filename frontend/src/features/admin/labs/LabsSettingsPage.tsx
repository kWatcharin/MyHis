import { labConfig } from '../master/configs'
import { MasterSettingsPage } from '../master/MasterSettingsPage'

export function LabsSettingsPage() {
  return <MasterSettingsPage config={labConfig} />
}
