import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class InputNavDecodeEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 void dec(long x)throws Exception{
  Function f=getFunctionContaining(toAddr(x)); println(String.format("\n===0x%08X===",x));
  if(f==null){println("NO_FUNCTION");return;}
  println(f.getName()+" @ "+f.getEntryPoint());
  DecompileResults r=d.decompileFunction(f,120,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 public void run()throws Exception{
  d.openProgram(currentProgram);
  long[] a={0x0056df2bL,0x005a8889L,0x005ad4aeL,0x005d0cc8L};
  for(long x:a)dec(x);
  d.dispose();
 }
}