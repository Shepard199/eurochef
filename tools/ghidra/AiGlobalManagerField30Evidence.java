import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
public class AiGlobalManagerField30Evidence extends GhidraScript {
 @Override public void run() throws Exception {
  DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
  FunctionIterator it=currentProgram.getFunctionManager().getFunctions(true);
  while(it.hasNext()){
   Function f=it.next(); long e=f.getEntryPoint().getOffset();
   if(e<0x00456000L||e>=0x00456E00L) continue;
   DecompileResults r=d.decompileFunction(f,90,monitor);
   if(!r.decompileCompleted()||r.getDecompiledFunction()==null)continue;
   String c=r.getDecompiledFunction().getC();
   if(c.contains("+ 0x30")||c.contains("[0xc]")||c.contains("+ 0x2c")||c.contains("+ 0x34")){
    println("\n=== "+f.getName()+" "+f.getEntryPoint()+" ==="); println(c);
   }
  }
  d.dispose();
 }
}