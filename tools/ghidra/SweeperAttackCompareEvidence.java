import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class SweeperAttackCompareEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
  long[] a={0x0045F240L,0x00460380L};
  for(long x:a){Function f=getFunctionContaining(toAddr(x));println(String.format("\n===0x%08X===",x));if(f!=null){DecompileResults r=d.decompileFunction(f,90,monitor);if(r.decompileCompleted())println(r.getDecompiledFunction().getC());}}
  d.dispose();
 }
}