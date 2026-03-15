export const getThemeClasses = (theme: 'light' | 'dark') => ({
  // Backgrounds
  bgPrimary: theme === 'dark' ? 'bg-[#0a1628]' : 'bg-gray-950',
  bgSecondary: theme === 'dark' ? 'bg-[#0f1d35]' : 'backdrop-blur-xl bg-gray-900/30',
  bgCard: theme === 'dark' ? 'bg-[#0f1d35]' : 'backdrop-blur-xl bg-gray-800/40',
  bgNested: theme === 'dark' ? 'bg-[#0a1628]' : 'bg-gray-800/50',
  
  // Borders
  border: theme === 'dark' ? 'border-[#1a2942]' : 'border-purple-500/20',
  borderHover: theme === 'dark' ? 'hover:border-[#5ddad5]/30' : 'hover:border-cyan-500/40',
  
  // Text
  textPrimary: 'text-white',
  textSecondary: 'text-gray-400',
  textGradient: theme === 'dark' 
    ? 'bg-gradient-to-r from-[#5ddad5] to-[#4facf7] bg-clip-text text-transparent'
    : 'bg-gradient-to-r from-purple-400 via-pink-400 to-cyan-400 bg-clip-text text-transparent',
  
  // Buttons
  btnPrimary: theme === 'dark'
    ? 'bg-gradient-to-r from-[#5ddad5] to-[#4facf7] text-[#0a1628] hover:opacity-90'
    : 'bg-gradient-to-r from-purple-500 to-cyan-500 text-white hover:opacity-90 shadow-lg shadow-purple-500/30',
  btnSecondary: theme === 'dark'
    ? 'bg-[#0a1628] border border-[#1a2942] hover:opacity-80 text-gray-400'
    : 'bg-gray-700/50 border border-gray-600/30 hover:bg-gray-700/70 text-gray-300',
  
  // Inputs
  input: theme === 'dark'
    ? 'bg-[#0a1628] border-[#1a2942] focus:border-[#5ddad5]'
    : 'bg-gray-800/50 border-purple-500/30 focus:ring-2 focus:ring-cyan-500',
  
  // Active states
  activeTab: theme === 'dark' ? 'border-[#5ddad5] text-white' : 'border-cyan-400 text-cyan-400 bg-cyan-400/10',
  inactiveTab: theme === 'dark' 
    ? 'border-transparent text-gray-400 hover:text-white'
    : 'border-transparent text-gray-400 hover:text-purple-300 hover:bg-purple-500/5',
});
