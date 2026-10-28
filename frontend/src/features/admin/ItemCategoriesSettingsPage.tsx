import { itemCategoryConfig } from './master/configs'
import { MasterSettingsPage } from './master/MasterSettingsPage'

export function ItemCategoriesSettingsPage() {
  return <MasterSettingsPage config={itemCategoryConfig} />
}
