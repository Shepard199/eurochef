import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
public class InputC8UsersEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 void dec(long x)throws Exception{
  Function f=getFunctionContaining(toAddr(x)); println(String.format("\n===0x%08X===",x));
  if(f==null){println("NO_FUNCTION");return;}
  println(f.getName()+" @ "+f.getEntryPoint());
  DecompileResults r=d.decompileFunction(f,90,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 public void run()throws Exception{
  d.openProgram(currentProgram);
  long[] a={0x0043a120L,0x00439630L,0x00440bc0L,0x00431cb0L,0x004f1d63L,0x004f89d3L,0x004fc1f9L};
  for(long x:a)dec(x);
  d.dispose();
 }
}